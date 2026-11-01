//! A growable buffer for TL encoding.

use crate::error::{Error, Result};
use crate::types::{
    FIRST_LONG_STRING, Int128, Int256, MAX_LONG_STRING, TYPE_BOOL_FALSE, TYPE_BOOL_TRUE,
    TYPE_VECTOR, padding_for,
};

/// A growable, owned TL encoding target.
///
/// Encoding appends to the internal byte vector. The buffer can be inspected
/// ([`Writer::as_slice`]), taken ([`Writer::into_vec`]), cleared
/// ([`Writer::clear`]) or patched in place ([`Writer::patch_u32`]).
#[derive(Debug, Default, Clone)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// Creates an empty writer.
    #[must_use]
    pub const fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Creates an empty writer with the given capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    /// Returns the number of bytes written.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns `true` when nothing has been written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Returns the written bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// Consumes the writer and returns the written bytes.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    /// Clears the buffer, keeping its capacity.
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// Reserves capacity for at least `additional` more bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(additional);
    }

    /// Appends raw bytes without any TL framing.
    pub fn put_raw(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Appends a single byte.
    pub fn put_u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    /// Appends a 32-bit little-endian unsigned integer.
    pub fn put_u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a 32-bit little-endian signed integer.
    pub fn put_i32(&mut self, value: i32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a 64-bit little-endian unsigned integer.
    pub fn put_u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a 64-bit little-endian signed integer.
    pub fn put_i64(&mut self, value: i64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a 64-bit little-endian IEEE-754 double.
    pub fn put_f64(&mut self, value: f64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a TL `bool`.
    pub fn put_bool(&mut self, value: bool) {
        self.put_u32(if value {
            TYPE_BOOL_TRUE
        } else {
            TYPE_BOOL_FALSE
        });
    }

    /// Appends a constructor identifier (an alias for [`Writer::put_u32`]).
    pub fn put_id(&mut self, id: u32) {
        self.put_u32(id);
    }

    /// Appends a 128-bit TL integer.
    pub fn put_int128(&mut self, value: Int128) {
        self.put_raw(value.as_array());
    }

    /// Appends a 256-bit TL integer.
    pub fn put_int256(&mut self, value: Int256) {
        self.put_raw(value.as_array());
    }

    /// Writes a TL length prefix for a payload of `len` bytes and returns the
    /// number of bytes the prefix occupied (1 or 4).
    fn put_len(&mut self, len: usize) -> usize {
        if len <= crate::types::MAX_SMALL_STRING {
            self.put_u8(len as u8);
            1
        } else {
            self.put_u8(FIRST_LONG_STRING);
            self.buf.push((len & 0xff) as u8);
            self.buf.push(((len >> 8) & 0xff) as u8);
            self.buf.push(((len >> 16) & 0xff) as u8);
            4
        }
    }

    /// Writes zero padding so that `total` bytes are aligned to a 4-byte
    /// boundary.
    fn put_padding(&mut self, total: usize) {
        let pad = padding_for(total);
        if pad > 0 {
            self.buf.resize(self.buf.len() + pad, 0);
        }
    }

    /// Appends a TL byte string.
    ///
    /// # Panics
    ///
    /// Panics if `bytes` is longer than [`MAX_LONG_STRING`]. Use
    /// [`Writer::put_bytes_checked`] for data of untrusted length.
    pub fn put_bytes(&mut self, bytes: &[u8]) {
        assert!(
            bytes.len() <= MAX_LONG_STRING,
            "byte string exceeds the maximum TL length"
        );
        let prefix = self.put_len(bytes.len());
        self.put_raw(bytes);
        self.put_padding(prefix + bytes.len());
    }

    /// Appends a TL byte string, returning an error instead of panicking when
    /// the payload is too long.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLength`] if `bytes` is longer than
    /// [`MAX_LONG_STRING`].
    pub fn put_bytes_checked(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_LONG_STRING {
            return Err(Error::InvalidLength {
                length: bytes.len(),
                context: "bytes",
            });
        }
        self.put_bytes(bytes);
        Ok(())
    }

    /// Appends a TL string.
    ///
    /// # Panics
    ///
    /// Panics if the string is longer than [`MAX_LONG_STRING`] bytes.
    pub fn put_string(&mut self, value: &str) {
        self.put_bytes(value.as_bytes());
    }

    /// Appends a TL string, returning an error instead of panicking when the
    /// value is too long.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLength`] if the string is longer than
    /// [`MAX_LONG_STRING`] bytes.
    pub fn put_string_checked(&mut self, value: &str) -> Result<()> {
        self.put_bytes_checked(value.as_bytes())
    }

    /// Appends a TL vector header for `count` elements.
    ///
    /// # Panics
    ///
    /// Panics if `count` does not fit in a non-negative 32-bit integer.
    pub fn put_vector_header(&mut self, count: usize) {
        assert!(
            count <= i32::MAX as usize,
            "vector element count does not fit in an int"
        );
        self.put_id(TYPE_VECTOR);
        self.put_i32(count as i32);
    }

    /// Overwrites four bytes at `offset` with a little-endian 32-bit value.
    ///
    /// Used to patch a length placeholder after the payload length is known.
    ///
    /// # Panics
    ///
    /// Panics if `offset + 4` is beyond the current buffer length.
    pub fn patch_u32(&mut self, offset: usize, value: u32) {
        let bytes = value.to_le_bytes();
        self.buf[offset..offset + 4].copy_from_slice(&bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::Writer;
    use crate::types::{TYPE_BOOL_TRUE, TYPE_VECTOR};

    #[test]
    fn integers_are_little_endian() {
        let mut w = Writer::new();
        w.put_u32(1);
        w.put_i64(-1);
        assert_eq!(
            w.as_slice(),
            &[1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
        );
    }

    #[test]
    fn bool_uses_constructor_ids() {
        let mut w = Writer::new();
        w.put_bool(true);
        assert_eq!(w.as_slice(), &TYPE_BOOL_TRUE.to_le_bytes());
    }

    #[test]
    fn string_padding() {
        let mut w = Writer::new();
        w.put_string("abc");
        assert_eq!(w.as_slice(), &[3, b'a', b'b', b'c']);
        w.clear();
        w.put_string("abcd");
        assert_eq!(w.as_slice(), &[4, b'a', b'b', b'c', b'd', 0, 0, 0]);
    }

    #[test]
    fn long_string_form() {
        let mut w = Writer::new();
        let payload = vec![0x41u8; 254];
        w.put_bytes(&payload);
        let out = w.as_slice();
        assert_eq!(out[0], 254);
        assert_eq!(&out[1..4], &[254, 0, 0]);
        assert_eq!(out.len(), 4 + 254 + 2);
    }

    #[test]
    fn checked_rejects_oversized() {
        let mut w = Writer::new();
        let payload = vec![0u8; crate::types::MAX_LONG_STRING + 1];
        assert!(w.put_bytes_checked(&payload).is_err());
    }

    #[test]
    fn vector_header() {
        let mut w = Writer::new();
        w.put_vector_header(2);
        let mut expected = TYPE_VECTOR.to_le_bytes().to_vec();
        expected.extend_from_slice(&2i32.to_le_bytes());
        assert_eq!(w.as_slice(), expected.as_slice());
    }

    #[test]
    fn patch_rewrites_in_place() {
        let mut w = Writer::new();
        w.put_u32(0);
        w.put_raw(&[1, 2, 3, 4]);
        w.patch_u32(0, w.len() as u32 - 4);
        assert_eq!(w.as_slice(), &[4, 0, 0, 0, 1, 2, 3, 4]);
    }
}
