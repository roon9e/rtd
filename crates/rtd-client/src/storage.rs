//! Session persistence backends.
//!
//! A [`Storage`] loads and stores the versioned [`SessionData`] blob so a login
//! survives a restart. [`FileStorage`] writes atomically (and with mode `0600`
//! on Unix); [`MemoryStorage`] keeps the session in memory for tests.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{Error, Result};
use crate::session::SessionData;

/// Loads and stores serialized session data.
pub trait Storage: Send + Sync {
    /// Returns the stored session, or `None` when none is present.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot be read or the blob is invalid.
    fn load(&self) -> Result<Option<SessionData>>;

    /// Persists `data`.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot be written.
    fn store(&self, data: &SessionData) -> Result<()>;

    /// Removes any stored session.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot be updated.
    fn delete(&self) -> Result<()>;
}

/// A file backend for the versioned [`SessionData`] blob.
#[derive(Debug, Clone)]
pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
    /// Creates a backend storing the session at `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the configured path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Storage for FileStorage {
    fn load(&self) -> Result<Option<SessionData>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(Some(SessionData::decode(&bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::Io(error)),
        }
    }

    fn store(&self, data: &SessionData) -> Result<()> {
        let temporary = self.path.with_extension("tmp");
        std::fs::write(&temporary, data.encode())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&temporary, &self.path)?;
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Io(error)),
        }
    }
}

/// A session imported from a foreign format.
///
/// Foreign formats (for example Telethon) carry the datacenter, address and
/// auth key but not the session id or salt, which are re-established on
/// reconnect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSession {
    /// The datacenter id.
    pub dc: i32,
    /// The `host:port` address.
    pub address: String,
    /// The raw 2048-bit authorization key.
    pub auth_key: [u8; 256],
}

impl ImportedSession {
    /// Converts the imported fields into [`SessionData`] with a fresh
    /// session id and salt.
    #[must_use]
    pub fn session_data(&self) -> SessionData {
        SessionData {
            auth_key: self.auth_key,
            session_id: 0,
            salt: 0,
        }
    }
}

/// Decodes a Telethon `StringSession` into an [`ImportedSession`].
///
/// The format is `<version>1` followed by URL-safe base64 of
/// `dc(1) ++ ip(4 or 16) ++ port(2, big-endian) ++ auth_key(256)`.
///
/// # Errors
///
/// Returns [`Error::Decode`] when the version, base64, length or address is
/// invalid.
pub fn decode_telethon(session: &str) -> Result<ImportedSession> {
    let version = session.chars().next();
    if version != Some('1') {
        return Err(Error::Decode(
            "unsupported Telethon session version".to_owned(),
        ));
    }
    let data = decode_base64_url(&session[1..])?;
    let (ip_len, address) = match data.len() {
        263 => (
            4,
            format!("{}.{}.{}.{}", data[1], data[2], data[3], data[4]),
        ),
        275 => (16, format_ipv6(&data[1..17])),
        length => {
            return Err(Error::Decode(format!(
                "Telethon session has invalid length {length}"
            )));
        }
    };
    let port = u16::from_be_bytes([data[1 + ip_len], data[2 + ip_len]]);
    let mut auth_key = [0u8; 256];
    auth_key.copy_from_slice(&data[3 + ip_len..3 + ip_len + 256]);
    let host = if ip_len == 16 {
        format!("[{address}]")
    } else {
        address
    };
    Ok(ImportedSession {
        dc: i32::from(data[0]),
        address: format!("{host}:{port}"),
        auth_key,
    })
}

/// Decodes URL-safe base64 (padding optional).
fn decode_base64_url(input: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in input.bytes() {
        if byte == b'=' || byte == b'\n' || byte == b'\r' {
            continue;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(Error::Decode("invalid base64url character".to_owned())),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

/// Formats 16 address bytes as a compressed IPv6 string.
fn format_ipv6(bytes: &[u8]) -> String {
    let mut groups = [0u16; 8];
    for (index, group) in groups.iter_mut().enumerate() {
        *group = u16::from_be_bytes([bytes[index * 2], bytes[index * 2 + 1]]);
    }
    // Find the longest run of zero groups (length >= 2) to compress.
    let mut best_start = 0usize;
    let mut best_len = 0usize;
    let mut run_start = 0usize;
    let mut run_len = 0usize;
    for (index, group) in groups.iter().enumerate() {
        if *group == 0 {
            if run_len == 0 {
                run_start = index;
            }
            run_len += 1;
            if run_len > best_len {
                best_len = run_len;
                best_start = run_start;
            }
        } else {
            run_len = 0;
        }
    }
    if best_len < 2 {
        return groups
            .iter()
            .map(|group| format!("{group:x}"))
            .collect::<Vec<_>>()
            .join(":");
    }
    let head = groups[..best_start]
        .iter()
        .map(|group| format!("{group:x}"))
        .collect::<Vec<_>>()
        .join(":");
    let tail = groups[best_start + best_len..]
        .iter()
        .map(|group| format!("{group:x}"))
        .collect::<Vec<_>>()
        .join(":");
    format!("{head}::{tail}")
}

/// An in-memory backend.
#[derive(Debug, Default)]
pub struct MemoryStorage {
    data: Mutex<Option<SessionData>>,
}

impl MemoryStorage {
    /// Creates an empty in-memory backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for MemoryStorage {
    fn load(&self) -> Result<Option<SessionData>> {
        Ok(*self
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    fn store(&self, data: &SessionData) -> Result<()> {
        *self
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(*data);
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        *self
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FileStorage, MemoryStorage, Storage, decode_telethon};
    use crate::session::SessionData;

    /// Encodes bytes as padded URL-safe base64 (for building Telethon sessions).
    fn encode_base64_url(data: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b0 = u32::from(chunk[0]);
            let b1 = chunk.get(1).copied().map(u32::from);
            let b2 = chunk.get(2).copied().map(u32::from);
            let triple = (b0 << 16) | (b1.unwrap_or(0) << 8) | b2.unwrap_or(0);
            out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
            out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
            out.push(if b1.is_some() {
                ALPHABET[((triple >> 6) & 0x3f) as usize] as char
            } else {
                '='
            });
            out.push(if b2.is_some() {
                ALPHABET[(triple & 0x3f) as usize] as char
            } else {
                '='
            });
        }
        out
    }

    #[test]
    fn decodes_telethon_sessions() {
        let key = [b'a'; 256];

        let mut ipv4 = vec![2u8, 192, 168, 0, 1, 0x01, 0xbb];
        ipv4.extend_from_slice(&key);
        let session = format!("1{}", encode_base64_url(&ipv4));
        let imported = decode_telethon(&session).unwrap();
        assert_eq!(imported.dc, 2);
        assert_eq!(imported.address, "192.168.0.1:443");
        assert_eq!(imported.auth_key, key);

        let mut ipv6 = vec![2u8, 0x20, 0x01, 0x0d, 0xb8];
        ipv6.extend_from_slice(&[0u8; 12]);
        ipv6.extend_from_slice(&[0x01, 0xbb]);
        ipv6.extend_from_slice(&key);
        let session = format!("1{}", encode_base64_url(&ipv6));
        let imported = decode_telethon(&session).unwrap();
        assert_eq!(imported.dc, 2);
        assert_eq!(imported.address, "[2001:db8::]:443");
    }

    #[test]
    fn rejects_invalid_telethon_sessions() {
        assert!(decode_telethon("").is_err());
        assert!(decode_telethon("2AAAA").is_err());
        assert!(decode_telethon(&format!("1{}", encode_base64_url(&[0u8; 10]))).is_err());
    }

    fn sample() -> SessionData {
        SessionData {
            auth_key: [9u8; 256],
            session_id: 0x1122_3344,
            salt: -7,
        }
    }

    #[test]
    fn memory_round_trips_and_deletes() {
        let storage = MemoryStorage::new();
        assert!(storage.load().unwrap().is_none());
        storage.store(&sample()).unwrap();
        assert_eq!(storage.load().unwrap(), Some(sample()));
        storage.delete().unwrap();
        assert!(storage.load().unwrap().is_none());
    }

    #[test]
    fn file_round_trips_and_deletes() {
        let path = std::env::temp_dir().join(format!(
            "rtd-client-storage-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let storage = FileStorage::new(&path);
        assert!(storage.load().unwrap().is_none());
        storage.store(&sample()).unwrap();
        assert_eq!(storage.load().unwrap(), Some(sample()));
        storage.delete().unwrap();
        assert!(storage.load().unwrap().is_none());
    }
}
