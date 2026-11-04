//! Telegram Desktop `tdata` session importer.
//!
//! Reads a `tdata` directory (the `key_data*` and `data*` files) and returns the
//! accounts it holds, decrypting the storage files with the passcode. This
//! mirrors the reference `td/session/tdesktop` implementation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pbkdf2::pbkdf2_hmac;
use sha1::{Digest, Sha1};
use sha2::Sha512;

use crate::md5::md5;

/// The `key_data` magic.
const MAGIC: [u8; 4] = *b"TDF$";
/// Salt length in `key_data`.
const SALT_LEN: usize = 32;
/// PBKDF2 rounds with a passcode.
const STRONG_ITERATIONS: u32 = 100_000;
/// PBKDF2 rounds without a passcode.
const WEAK_ITERATIONS: u32 = 1;
/// `dbiMtpAuthorization` tag.
const DBI_MTP_AUTHORIZATION: u32 = 0x4b;

/// A Telegram Desktop import failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdesktopError(pub String);

impl core::fmt::Display for TdesktopError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for TdesktopError {}

impl From<std::io::Error> for TdesktopError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

/// An account found in a `tdata` directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// The internal Telegram Desktop account index.
    pub idx: u32,
    /// The Telegram user id.
    pub user_id: u64,
    /// The main datacenter id.
    pub main_dc: i32,
    /// The authorization key per datacenter.
    pub keys: BTreeMap<i32, [u8; 256]>,
}

/// Reads the accounts from a `tdata` directory with `passcode`.
///
/// # Errors
///
/// Returns [`TdesktopError`] when the directory or its files cannot be read or
/// decrypted.
pub fn read(root: impl AsRef<Path>, passcode: &[u8]) -> Result<Vec<Account>, TdesktopError> {
    let root = root.as_ref();
    let key_data = read_tdf_file(&open_file(root, "key_data")?)?;
    let (local_key, accounts) = read_key_data(&key_data, passcode)?;
    if accounts.is_empty() {
        return Err(TdesktopError("tdata has no accounts".to_owned()));
    }

    let mut result = Vec::with_capacity(accounts.len());
    for account in accounts {
        let name = if account > 0 {
            format!("data#{}", account + 1)
        } else {
            "data".to_owned()
        };
        let data_file = read_tdf_file(&open_file(root, &name)?)?;
        result.push(read_mtp_data(&data_file, &local_key, account)?);
    }
    Ok(result)
}

/// Opens `name` with the `s`/`0`/`1` suffixes, preferring the modern `s` file.
fn open_file(root: &Path, name: &str) -> Result<Vec<u8>, TdesktopError> {
    for suffix in ["s", "0", "1"] {
        let path: PathBuf = root.join(format!("{name}{suffix}"));
        if let Ok(bytes) = std::fs::read(&path) {
            return Ok(bytes);
        }
    }
    Err(TdesktopError(format!("file {name} not found")))
}

/// A parsed `TDF$` storage file.
struct TdfFile {
    data: Vec<u8>,
    pos: usize,
}

/// Validates the header and hash of a `TDF$` file.
fn read_tdf_file(bytes: &[u8]) -> Result<TdfFile, TdesktopError> {
    if bytes.len() < 8 + 16 {
        return Err(TdesktopError("tdesktop file too small".to_owned()));
    }
    if bytes[..4] != MAGIC {
        return Err(TdesktopError("wrong tdesktop file magic".to_owned()));
    }
    let version: [u8; 4] = bytes[4..8].try_into().expect("four bytes");
    let body = &bytes[8..bytes.len() - 16];
    let hash = &bytes[bytes.len() - 16..];

    let mut input = body.to_vec();
    input.extend_from_slice(&(body.len() as u32).to_le_bytes());
    input.extend_from_slice(&version);
    input.extend_from_slice(&MAGIC);
    if md5(&input) != hash {
        return Err(TdesktopError("tdesktop file hash mismatch".to_owned()));
    }
    Ok(TdfFile {
        data: body.to_vec(),
        pos: 0,
    })
}

impl TdfFile {
    /// Reads a Qt `QByteArray` (big-endian length prefix).
    fn read_array(&mut self) -> Result<Vec<u8>, TdesktopError> {
        if self.pos + 4 > self.data.len() {
            return Err(TdesktopError("truncated array".to_owned()));
        }
        let length = u32::from_be_bytes(self.data[self.pos..self.pos + 4].try_into().expect("four"))
            as usize;
        self.pos += 4;
        if length == 0xffff_ffff {
            return Ok(Vec::new());
        }
        if self.pos + length > self.data.len() {
            return Err(TdesktopError("truncated array body".to_owned()));
        }
        let value = self.data[self.pos..self.pos + length].to_vec();
        self.pos += length;
        Ok(value)
    }
}

/// Reads `key_data`, returning the local key and the account indices.
fn read_key_data(file: &TdfFile, passcode: &[u8]) -> Result<([u8; 256], Vec<u32>), TdesktopError> {
    let mut file = TdfFile {
        data: file.data.clone(),
        pos: 0,
    };
    let salt = file.read_array()?;
    if salt.len() != SALT_LEN {
        return Err(TdesktopError(format!("invalid salt length {}", salt.len())));
    }
    let passcode_key = create_local_key(passcode, &salt);

    let key_encrypted = file.read_array()?;
    let key_inner = decrypt_local(&key_encrypted, &passcode_key)?;
    let (key, _) = read_array_be(&key_inner)?;
    if key.len() < 256 {
        return Err(TdesktopError(format!("key too small ({})", key.len())));
    }
    let mut local_key = [0u8; 256];
    local_key.copy_from_slice(&key[..256]);

    let info_encrypted = file.read_array()?;
    let info = decrypt_local(&info_encrypted, &local_key)?;
    let info = &info[4..];
    if info.len() < 4 {
        return Err(TdesktopError("truncated key info".to_owned()));
    }
    let count = u32::from_be_bytes(info[..4].try_into().expect("four")) as usize;
    let body = &info[4..];
    if body.len() < count * 4 {
        return Err(TdesktopError("truncated account list".to_owned()));
    }
    let accounts = (0..count)
        .map(|i| u32::from_be_bytes(body[i * 4..i * 4 + 4].try_into().expect("four")))
        .collect();
    Ok((local_key, accounts))
}

/// Reads `data`, returning the account's authorization.
fn read_mtp_data(
    file: &TdfFile,
    local_key: &[u8; 256],
    idx: u32,
) -> Result<Account, TdesktopError> {
    let mut file = TdfFile {
        data: file.data.clone(),
        pos: 0,
    };
    let encrypted = file.read_array()?;
    let decrypted = decrypt_local(&encrypted, local_key)?;
    let mut reader = QtReader::new(&decrypted[4..]);

    if reader.u32()? != DBI_MTP_AUTHORIZATION {
        return Err(TdesktopError("unexpected dbi id".to_owned()));
    }
    reader.skip(4)?;
    let legacy_user = reader.u32()?;
    let legacy_dc = reader.u32()?;
    let (user_id, main_dc) = if (u64::from(legacy_user) << 32) | u64::from(legacy_dc) == u64::MAX {
        let user = reader.u64()?;
        let dc = reader.u32()?;
        (user, dc as i32)
    } else {
        (u64::from(legacy_user), legacy_dc as i32)
    };

    let count = reader.u32()? as usize;
    let mut keys = BTreeMap::new();
    for _ in 0..count {
        let dc = reader.u32()? as i32;
        let mut key = [0u8; 256];
        reader.consume(&mut key)?;
        keys.insert(dc, key);
    }
    Ok(Account {
        idx,
        user_id,
        main_dc,
        keys,
    })
}

/// Derives the local key from a passcode and salt (`PBKDF2-HMAC-SHA512`).
fn create_local_key(passcode: &[u8], salt: &[u8]) -> [u8; 256] {
    let iterations = if passcode.is_empty() {
        WEAK_ITERATIONS
    } else {
        STRONG_ITERATIONS
    };
    let mut hasher = Sha512::new();
    hasher.update(salt);
    hasher.update(passcode);
    hasher.update(salt);
    let seed = hasher.finalize();

    let mut key = [0u8; 256];
    pbkdf2_hmac::<Sha512>(&seed, salt, iterations, &mut key);
    key
}

/// Decrypts a `decryptLocal`-framed payload with `local_key`.
fn decrypt_local(encrypted: &[u8], local_key: &[u8; 256]) -> Result<Vec<u8>, TdesktopError> {
    if !encrypted.len().is_multiple_of(16) {
        return Err(TdesktopError(
            "encrypted data is not 16-byte aligned".to_owned(),
        ));
    }
    if encrypted.len() < 16 {
        return Err(TdesktopError("encrypted data too small".to_owned()));
    }
    let msg_key: [u8; 16] = encrypted[..16].try_into().expect("sixteen bytes");
    let mut data = encrypted[16..].to_vec();
    let (key, iv) = old_keys_server(local_key, &msg_key);
    rtd_crypto::ige_decrypt(&key, &iv, &mut data);
    let digest = Sha1::digest(&data);
    if digest[..16] != msg_key {
        return Err(TdesktopError("local message key mismatch".to_owned()));
    }
    Ok(data)
}

/// MTProto 1.0 key schedule for the server side (`x = 8`).
fn old_keys_server(auth: &[u8; 256], msg_key: &[u8; 16]) -> ([u8; 32], [u8; 32]) {
    let x = 8usize;
    let a = sha1_parts(&[msg_key, &auth[x..x + 32]]);
    let b = sha1_parts(&[&auth[x + 32..x + 48], msg_key, &auth[x + 48..x + 64]]);
    let c = sha1_parts(&[&auth[x + 64..x + 96], msg_key]);
    let d = sha1_parts(&[msg_key, &auth[x + 96..x + 128]]);

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&a[0..8]);
    key[8..20].copy_from_slice(&b[8..20]);
    key[20..32].copy_from_slice(&c[4..16]);

    let mut iv = [0u8; 32];
    iv[0..12].copy_from_slice(&a[8..20]);
    iv[12..20].copy_from_slice(&b[0..8]);
    iv[20..24].copy_from_slice(&c[16..20]);
    iv[24..32].copy_from_slice(&d[0..8]);
    (key, iv)
}

/// `SHA1` over the concatenation of `parts`.
fn sha1_parts(parts: &[&[u8]]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// Reads a Qt `QByteArray` from a slice (big-endian length prefix).
fn read_array_be(data: &[u8]) -> Result<(Vec<u8>, usize), TdesktopError> {
    if data.len() < 4 {
        return Err(TdesktopError("truncated array".to_owned()));
    }
    let length = u32::from_be_bytes(data[..4].try_into().expect("four")) as usize;
    if length == 0xffff_ffff {
        return Ok((Vec::new(), 4));
    }
    if 4 + length > data.len() {
        return Err(TdesktopError("truncated array body".to_owned()));
    }
    Ok((data[4..4 + length].to_vec(), 4 + length))
}

/// A big-endian reader over a decrypted payload.
struct QtReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> QtReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], TdesktopError> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| TdesktopError("truncated qt value".to_owned()))?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, TdesktopError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().expect("four")))
    }

    fn u64(&mut self) -> Result<u64, TdesktopError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().expect("eight")))
    }

    fn skip(&mut self, len: usize) -> Result<(), TdesktopError> {
        self.take(len).map(|_| ())
    }

    fn consume(&mut self, target: &mut [u8]) -> Result<(), TdesktopError> {
        target.copy_from_slice(self.take(target.len())?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sha1::{Digest, Sha1};

    use super::{DBI_MTP_AUTHORIZATION, MAGIC, create_local_key, old_keys_server, read};

    #[test]
    fn local_key_is_deterministic() {
        let a = create_local_key(b"pass", &[7u8; 32]);
        let b = create_local_key(b"pass", &[7u8; 32]);
        assert_eq!(a, b);
        assert_ne!(a, create_local_key(b"other", &[7u8; 32]));
    }

    #[test]
    fn old_keys_server_is_deterministic() {
        let auth = [3u8; 256];
        let msg = [9u8; 16];
        let (k1, iv1) = old_keys_server(&auth, &msg);
        let (k2, iv2) = old_keys_server(&auth, &msg);
        assert_eq!(k1, k2);
        assert_eq!(iv1, iv2);
        assert_ne!(k1, iv1);
    }

    /// Wraps `body` as a `TDF$` file with its `md5` trailer.
    fn tdf_file(body: &[u8]) -> Vec<u8> {
        let version = [0u8; 4];
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&version);
        out.extend_from_slice(body);
        let mut input = body.to_vec();
        input.extend_from_slice(&(body.len() as u32).to_le_bytes());
        input.extend_from_slice(&version);
        input.extend_from_slice(&MAGIC);
        out.extend_from_slice(&crate::md5::md5(&input));
        out
    }

    /// A big-endian length-prefixed Qt array.
    fn put_array(data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(data);
        out
    }

    /// Inverse of `decrypt_local`: SHA-1 message key + AES-IGE over padded data.
    fn encrypt_local(plain: &[u8], local_key: &[u8; 256]) -> Vec<u8> {
        let mut padded = plain.to_vec();
        while !padded.len().is_multiple_of(16) {
            padded.push(0);
        }
        let digest = Sha1::digest(&padded);
        let mut msg_key = [0u8; 16];
        msg_key.copy_from_slice(&digest[..16]);
        let (key, iv) = old_keys_server(local_key, &msg_key);
        rtd_crypto::ige_encrypt(&key, &iv, &mut padded);
        let mut out = msg_key.to_vec();
        out.extend_from_slice(&padded);
        out
    }

    #[test]
    fn reads_a_synthetic_tdata_directory() {
        let passcode = b"passcode";
        let salt = [0x33u8; 32];
        let passcode_key = create_local_key(passcode, &salt);
        let local_key = [0x44u8; 256];

        // `key_data`: salt, the wrapped local key, then the account list.
        let key_inner = put_array(&local_key);
        let key_encrypted = encrypt_local(&key_inner, &passcode_key);
        let mut info_plain = vec![0u8; 4];
        info_plain.extend_from_slice(&1u32.to_be_bytes()); // account count
        info_plain.extend_from_slice(&0u32.to_be_bytes()); // account index 0
        let info_encrypted = encrypt_local(&info_plain, &local_key);
        let mut key_body = put_array(&salt);
        key_body.extend_from_slice(&put_array(&key_encrypted));
        key_body.extend_from_slice(&put_array(&info_encrypted));

        // `data`: the `dbiMtpAuthorization` record.
        let mut mtp_plain = vec![0u8; 4];
        mtp_plain.extend_from_slice(&DBI_MTP_AUTHORIZATION.to_be_bytes());
        mtp_plain.extend_from_slice(&[0u8; 4]);
        mtp_plain.extend_from_slice(&123u32.to_be_bytes()); // legacy user id
        mtp_plain.extend_from_slice(&2u32.to_be_bytes()); // legacy dc id
        mtp_plain.extend_from_slice(&1u32.to_be_bytes()); // key count
        mtp_plain.extend_from_slice(&2u32.to_be_bytes()); // dc id
        mtp_plain.extend_from_slice(&[0x55u8; 256]); // authorization key
        let mtp_encrypted = encrypt_local(&mtp_plain, &local_key);

        let dir = std::env::temp_dir().join(format!("rtd-tdesktop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("key_datas"), tdf_file(&key_body)).expect("write key_data");
        std::fs::write(dir.join("datas"), tdf_file(&put_array(&mtp_encrypted)))
            .expect("write data");

        let accounts = read(&dir, passcode).expect("read tdata");
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(accounts.len(), 1);
        let account = &accounts[0];
        assert_eq!(account.idx, 0);
        assert_eq!(account.user_id, 123);
        assert_eq!(account.main_dc, 2);
        assert_eq!(account.keys.get(&2).copied(), Some([0x55u8; 256]));
    }
}
