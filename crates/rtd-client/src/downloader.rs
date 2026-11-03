//! File downloads.
//!
//! [`Downloader`] fetches a file in ranges with `upload.getFile`, assembling the
//! parts into one buffer. CDN redirection is reported as unsupported (a
//! follow-up), matching the current single-DC runtime.

use rtd_tg::{InputFileLocation, Method, Object, UploadFile, UploadGetFile};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// Default download part size (a multiple of the TL 4 KiB alignment).
const DEFAULT_PART: usize = 512 * 1024;
/// Maximum part size accepted by Telegram.
const MAX_PART: usize = 1024 * 1024;
/// The TL alignment.
const ALIGNMENT: usize = 4096;

/// A ranged file downloader.
#[derive(Debug, Clone, Copy)]
pub struct Downloader {
    part_size: usize,
}

impl Downloader {
    /// Creates a downloader with the default part size.
    #[must_use]
    pub fn new() -> Self {
        Self {
            part_size: DEFAULT_PART,
        }
    }

    /// Overrides the part size, clamped to the alignment and Telegram's maximum.
    #[must_use]
    pub fn with_part_size(mut self, part_size: usize) -> Self {
        self.part_size = (part_size.clamp(ALIGNMENT, MAX_PART) / ALIGNMENT) * ALIGNMENT;
        self
    }

    /// Returns the configured part size.
    #[must_use]
    pub fn part_size(&self) -> usize {
        self.part_size
    }

    /// Downloads `size` bytes of `location` and returns them.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with an unexpected constructor or a CDN redirect.
    pub fn download<T: Transport>(
        &self,
        client: &mut Client<T>,
        location: InputFileLocation,
        size: usize,
    ) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(size.min(64 * 1024 * 1024));
        let mut offset: i64 = 0;
        while out.len() < size {
            let remaining = size - out.len();
            let limit = i32::try_from(remaining.min(self.part_size).max(1))
                .map_err(|_| Error::Unexpected("download part too large".to_owned()))?;
            let query = Method::UploadGetFile(Box::new(UploadGetFile {
                flags: 0,
                precise: false,
                cdn_supported: false,
                location: Box::new(location.clone()),
                offset,
                limit,
            }));
            match client.invoke(query)? {
                Object::UploadFile(file) => match *file {
                    UploadFile::File { bytes, .. } => {
                        if bytes.is_empty() {
                            break;
                        }
                        offset += bytes.len() as i64;
                        out.extend_from_slice(&bytes);
                    }
                    UploadFile::FileCdnRedirect { .. } => {
                        return Err(Error::Unexpected(
                            "upload.getFile returned a CDN redirect, which is not supported"
                                .to_owned(),
                        ));
                    }
                },
                other => {
                    return Err(Error::Unexpected(format!(
                        "upload.getFile returned unexpected constructor {:#010x}",
                        other.type_id()
                    )));
                }
            }
        }
        Ok(out)
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Downloader, MAX_PART};

    #[test]
    fn part_size_is_aligned_and_clamped() {
        assert_eq!(Downloader::new().part_size(), 512 * 1024);
        assert_eq!(Downloader::new().with_part_size(1000).part_size(), 4096);
        assert_eq!(
            Downloader::new()
                .with_part_size(5 * 1024 * 1024)
                .part_size(),
            MAX_PART
        );
        assert_eq!(
            Downloader::new().with_part_size(300_000).part_size() % 4096,
            0
        );
    }
}
