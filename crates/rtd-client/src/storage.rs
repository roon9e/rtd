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
    use super::{FileStorage, MemoryStorage, Storage};
    use crate::session::SessionData;

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
