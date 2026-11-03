//! Edge-resident file-data serving.
//!
//! A [`FileService`] serves file ranges directly from the edge, so
//! `upload.getFile` need not round-trip to the core. [`MemoryFileService`] is an
//! in-memory implementation for tests and single-process deployments.

use std::collections::HashMap;
use std::sync::Mutex;

/// A ranged file request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRequest {
    /// The opaque file location key.
    pub location: Vec<u8>,
    /// The byte offset to read from.
    pub offset: i64,
    /// The maximum number of bytes to return.
    pub limit: i32,
}

/// A file-service error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileError(pub String);

impl core::fmt::Display for FileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for FileError {}

/// Serves file ranges.
pub trait FileService: Send + Sync {
    /// Returns up to `limit` bytes of `location` starting at `offset`.
    ///
    /// # Errors
    ///
    /// Returns [`FileError`] when the location is unknown or the range is
    /// invalid.
    fn get_file(&self, request: &FileRequest) -> Result<Vec<u8>, FileError>;
}

/// An in-memory file service.
#[derive(Debug, Default)]
pub struct MemoryFileService {
    files: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
}

impl MemoryFileService {
    /// Creates an empty service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `data` under `location`.
    pub fn put(&self, location: Vec<u8>, data: Vec<u8>) {
        self.files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(location, data);
    }
}

impl FileService for MemoryFileService {
    fn get_file(&self, request: &FileRequest) -> Result<Vec<u8>, FileError> {
        let files = self
            .files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let data = files
            .get(&request.location)
            .ok_or_else(|| FileError("unknown file location".to_owned()))?;
        let start =
            usize::try_from(request.offset).map_err(|_| FileError("negative offset".to_owned()))?;
        if start > data.len() {
            return Err(FileError("offset past end of file".to_owned()));
        }
        let limit = usize::try_from(request.limit.max(0))
            .map_err(|_| FileError("invalid limit".to_owned()))?;
        let end = (start + limit).min(data.len());
        Ok(data[start..end].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::{FileRequest, FileService, MemoryFileService};

    fn request(location: &[u8], offset: i64, limit: i32) -> FileRequest {
        FileRequest {
            location: location.to_vec(),
            offset,
            limit,
        }
    }

    #[test]
    fn serves_ranges() {
        let service = MemoryFileService::new();
        service.put(b"file-1".to_vec(), (0..100u8).collect());
        assert_eq!(
            service.get_file(&request(b"file-1", 10, 5)).unwrap(),
            (10..15u8).collect::<Vec<u8>>()
        );
        // A limit past the end is clamped.
        assert_eq!(
            service
                .get_file(&request(b"file-1", 95, 100))
                .unwrap()
                .len(),
            5
        );
    }

    #[test]
    fn rejects_unknown_and_invalid() {
        let service = MemoryFileService::new();
        assert!(service.get_file(&request(b"missing", 0, 1)).is_err());
        service.put(b"file-2".to_vec(), vec![1, 2, 3]);
        assert!(service.get_file(&request(b"file-2", 100, 1)).is_err());
    }
}
