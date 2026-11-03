//! File uploads.
//!
//! [`Uploader`] splits a byte slice into parts and uploads it with
//! `upload.saveFilePart` (small files, with an MD5 checksum) or
//! `upload.saveBigFilePart` (large files), growing the part size so the part
//! count stays within the Telegram limit.

use rtd_tg::{InputFile, Method, Object, UploadSaveBigFilePart, UploadSaveFilePart};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::md5::{hex, md5};
use crate::transport::Transport;

/// Largest file uploaded with `saveFilePart`.
const SMALL_MAX: usize = 10 * 1024 * 1024;
/// Maximum number of parts a file may be split into.
const MAX_PARTS: usize = 3999;
/// Default part size.
const DEFAULT_PART: usize = 512 * 1024;
/// Minimum part size (the TL 4 KiB alignment).
const MIN_PART: usize = 4096;

/// A source of bytes to upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// An in-memory buffer.
    Bytes(Vec<u8>),
}

impl Source {
    /// Creates a byte source.
    #[must_use]
    pub fn bytes(data: impl Into<Vec<u8>>) -> Self {
        Self::Bytes(data.into())
    }

    /// Returns the source bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Bytes(data) => data,
        }
    }
}

impl From<Vec<u8>> for Source {
    fn from(data: Vec<u8>) -> Self {
        Self::Bytes(data)
    }
}

/// Reports upload progress.
pub trait Progress: Send {
    /// Reports `uploaded` of `total` bytes.
    fn progress(&self, uploaded: usize, total: usize);
}

/// A [`Progress`] that does nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoProgress;

impl Progress for NoProgress {
    fn progress(&self, _uploaded: usize, _total: usize) {}
}

/// An uploader with a configurable part size.
#[derive(Debug, Clone, Copy, Default)]
pub struct Uploader {
    part_size: Option<usize>,
}

impl Uploader {
    /// Creates an uploader with the default part size.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the part size (rounded up to the TL alignment).
    #[must_use]
    pub fn with_part_size(mut self, part_size: usize) -> Self {
        self.part_size = Some(part_size);
        self
    }

    /// Returns the part size used for a file of `len` bytes.
    ///
    /// The size grows with the file so the part count stays within
    /// [`MAX_PARTS`], and is always a multiple of [`MIN_PART`].
    #[must_use]
    pub fn part_size(&self, len: usize) -> usize {
        let base = self
            .part_size
            .unwrap_or_else(|| len.div_ceil(MAX_PARTS).max(DEFAULT_PART));
        base.max(MIN_PART).div_ceil(MIN_PART) * MIN_PART
    }

    /// Uploads `data` under `name`, returning the `InputFile` to reference it.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server does not acknowledge a part with `Bool`.
    pub fn upload<T: Transport>(
        &self,
        client: &mut Client<T>,
        name: &str,
        data: &[u8],
    ) -> Result<InputFile> {
        self.upload_with_progress(client, name, data, &NoProgress)
    }

    /// Uploads every byte read from `reader` under `name`.
    ///
    /// The reader is drained into memory before uploading; use this for
    /// file-backed sources.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the reader fails, otherwise see
    /// [`Uploader::upload`].
    pub fn upload_reader<T: Transport>(
        &self,
        client: &mut Client<T>,
        name: &str,
        mut reader: impl std::io::Read,
        progress: &dyn Progress,
    ) -> Result<InputFile> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        self.upload_with_progress(client, name, &data, progress)
    }

    /// Uploads `data`, reporting progress after each part.
    ///
    /// # Errors
    ///
    /// See [`Uploader::upload`].
    pub fn upload_with_progress<T: Transport>(
        &self,
        client: &mut Client<T>,
        name: &str,
        data: &[u8],
        progress: &dyn Progress,
    ) -> Result<InputFile> {
        let file_id = random_id();
        let part_size = self.part_size(data.len());
        let chunks: Vec<&[u8]> = data.chunks(part_size).collect();
        let parts = i32::try_from(chunks.len())
            .map_err(|_| Error::Unexpected("file has more parts than fit in an i32".to_owned()))?;
        let total = data.len();
        let mut uploaded = 0usize;

        if data.len() <= SMALL_MAX {
            for (index, chunk) in chunks.iter().enumerate() {
                let part = i32::try_from(index)
                    .map_err(|_| Error::Unexpected("part index overflow".to_owned()))?;
                let query = Method::UploadSaveFilePart(Box::new(UploadSaveFilePart {
                    file_id,
                    file_part: part,
                    bytes: chunk.to_vec(),
                }));
                expect_bool(client.invoke(query)?)?;
                uploaded += chunk.len();
                progress.progress(uploaded, total);
            }
            Ok(InputFile::InputFile {
                id: file_id,
                parts,
                name: name.to_owned(),
                md5_checksum: hex(&md5(data)),
            })
        } else {
            for (index, chunk) in chunks.iter().enumerate() {
                let part = i32::try_from(index)
                    .map_err(|_| Error::Unexpected("part index overflow".to_owned()))?;
                let query = Method::UploadSaveBigFilePart(Box::new(UploadSaveBigFilePart {
                    file_id,
                    file_part: part,
                    file_total_parts: parts,
                    bytes: chunk.to_vec(),
                }));
                expect_bool(client.invoke(query)?)?;
                uploaded += chunk.len();
                progress.progress(uploaded, total);
            }
            Ok(InputFile::InputFileBig {
                id: file_id,
                parts,
                name: name.to_owned(),
            })
        }
    }
}

/// Returns a random non-zero file identifier.
fn random_id() -> i64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&rtd_crypto::random_bytes(8));
    let id = i64::from_le_bytes(bytes);
    if id == 0 { 1 } else { id }
}

/// Requires a `Bool` acknowledgement from `object`.
fn expect_bool(object: Object) -> Result<()> {
    match object {
        Object::Bool(_) => Ok(()),
        other => Err(Error::Unexpected(format!(
            "upload part returned unexpected constructor {:#010x}",
            other.type_id()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PART, MAX_PARTS, Uploader};

    #[test]
    fn part_size_grows_to_bound_the_part_count() {
        let uploader = Uploader::new();
        assert_eq!(uploader.part_size(1024), DEFAULT_PART);
        // A huge file grows the part size so the count stays within the limit.
        let len = 20 * 1024 * 1024 * 1024usize; // 20 GiB
        let part = uploader.part_size(len);
        assert!(part >= len.div_ceil(MAX_PARTS));
        assert!(part.is_multiple_of(4096));
        assert!(len.div_ceil(part) <= MAX_PARTS);
    }

    #[test]
    fn explicit_part_size_is_aligned() {
        let uploader = Uploader::new().with_part_size(1000);
        assert_eq!(uploader.part_size(10), 4096);
    }
}
