//! Packed/gzipped MTProto payloads (`gzip#3072cfa1`).
//!
//! A packed object replaces any other serialized object with its gzipped
//! representation. Decoding is bounded to guard against decompression bombs.

use crate::error::{Error, Result};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use rtd_bin::{Reader, Writer};
use std::io::{Read, Write};

/// TL type id of [`Gzip`].
pub const GZIP_TYPE_ID: u32 = 0x3072_cfa1;

/// Maximum number of bytes a packed payload may expand to (10 MiB).
pub const MAX_DECOMPRESSED_SIZE: usize = 10 * 1024 * 1024;

/// A gzipped MTProto payload.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Gzip {
    /// Uncompressed payload.
    pub data: Vec<u8>,
}

impl Gzip {
    /// Creates a packed payload from uncompressed data.
    #[must_use]
    pub const fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Encodes the payload into a fresh buffer.
    ///
    /// The wire form is the type id followed by a TL byte string carrying the
    /// gzip-compressed data.
    ///
    /// # Errors
    ///
    /// Returns an error if compression fails or the compressed payload does not
    /// fit in a TL byte string.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&self.data)?;
        let compressed = encoder.finish()?;

        let mut writer = Writer::new();
        writer.put_id(GZIP_TYPE_ID);
        writer.put_bytes_checked(&compressed)?;
        Ok(writer.into_vec())
    }

    /// Decodes a packed payload, enforcing the expansion bound.
    ///
    /// # Errors
    ///
    /// Returns an error if the type id is wrong, the inner byte string is
    /// malformed, the gzip stream is invalid, or the payload expands to
    /// [`MAX_DECOMPRESSED_SIZE`] bytes or more.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input);
        reader.consume_id(GZIP_TYPE_ID)?;
        let compressed = reader.take_bytes()?;

        let decoder = GzDecoder::new(compressed);
        let mut limited = decoder.take(MAX_DECOMPRESSED_SIZE as u64);
        let mut data = Vec::new();
        limited.read_to_end(&mut data)?;

        if data.len() >= MAX_DECOMPRESSED_SIZE {
            return Err(Error::DecompressionBomb {
                limit: MAX_DECOMPRESSED_SIZE,
                decompressed: data.len(),
            });
        }

        Ok(Self { data })
    }

    /// Decodes a packed payload from a borrowed slice.
    ///
    /// # Errors
    ///
    /// See [`Gzip::decode`].
    pub fn from_bytes(input: &[u8]) -> Result<Self> {
        Self::decode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::{GZIP_TYPE_ID, Gzip, MAX_DECOMPRESSED_SIZE};

    #[test]
    fn round_trip() {
        let original = Gzip::new(b"hello, packed world".to_vec());
        let encoded = original.encode().unwrap();
        assert_eq!(
            u32::from_le_bytes(encoded[..4].try_into().unwrap()),
            GZIP_TYPE_ID
        );
        let decoded = Gzip::decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn round_trip_empty() {
        let original = Gzip::new(Vec::new());
        let encoded = original.encode().unwrap();
        assert_eq!(Gzip::decode(&encoded).unwrap(), original);
    }

    #[test]
    fn decompression_bomb_is_rejected() {
        // 11 MiB of zeros compresses to a tiny payload but would expand past
        // the 10 MiB limit.
        let bomb = Gzip::new(vec![0u8; MAX_DECOMPRESSED_SIZE + 1024 * 1024]);
        let encoded = bomb.encode().unwrap();
        assert!(matches!(
            Gzip::decode(&encoded),
            Err(super::Error::DecompressionBomb { .. })
        ));
    }

    #[test]
    fn wrong_type_id_is_rejected() {
        let encoded = Gzip::new(vec![1, 2, 3]).encode().unwrap();
        let mut corrupted = encoded;
        corrupted[0] ^= 0xff;
        assert!(Gzip::decode(&corrupted).is_err());
    }

    #[test]
    fn from_bytes_matches_decode() {
        let encoded = Gzip::new(vec![4, 5, 6]).encode().unwrap();
        assert_eq!(
            Gzip::from_bytes(&encoded).unwrap(),
            Gzip::decode(&encoded).unwrap()
        );
    }
}
