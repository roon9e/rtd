//! A borrowed cursor for TL decoding.

use crate::error::{Error, Result};
use crate::types::{
    FIRST_LONG_STRING, Int128, Int256, TYPE_BOOL_FALSE, TYPE_BOOL_TRUE, TYPE_VECTOR, padding_for,
};

/// A cursor over a borrowed byte slice.
///
/// The reader holds no owned data: it borrows the input and advances an
/// internal position. Byte-string and string values are returned as borrowed
/// slices of the input (zero-copy); callers that need an owned value can copy
/// with [`slice::to_vec`].
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Creates a reader positioned at the start of `input`.
    #[must_use]
    pub const fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }

    /// Returns the number of unread bytes.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.input.len() - self.pos
    }

    /// Returns `true` when no bytes remain.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.pos >= self.input.len()
    }

    /// Returns the current cursor position.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// Returns the whole underlying input.
    #[must_use]
    pub const fn input(&self) -> &'a [u8] {
        self.input
    }

    /// Returns the unread portion without advancing the cursor.
    #[must_use]
    pub fn peek_rest(&self) -> &'a [u8] {
        &self.input[self.pos..]
    }

    /// Returns an independent reader over the unread portion without advancing.
    #[must_use]
    pub fn sub_reader(&self) -> Reader<'a> {
        Reader::new(self.peek_rest())
    }

    /// Resets the cursor to the start.
    pub fn reset(&mut self) {
        self.pos = 0;
    }

    /// Moves the cursor to `pos`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedEof`] if `pos` is beyond the end of the input.
    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.input.len() {
            return Err(Error::UnexpectedEof {
                needed: pos,
                remaining: self.input.len(),
            });
        }
        self.pos = pos;
        Ok(())
    }

    /// Skips `n` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedEof`] if fewer than `n` bytes remain.
    pub fn skip(&mut self, n: usize) -> Result<()> {
        let end = self.pos.checked_add(n).ok_or(Error::UnexpectedEof {
            needed: n,
            remaining: self.remaining(),
        })?;
        if end > self.input.len() {
            return Err(Error::UnexpectedEof {
                needed: n,
                remaining: self.remaining(),
            });
        }
        self.pos = end;
        Ok(())
    }

    /// Reads `n` raw bytes, borrowing from the input.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedEof`] if fewer than `n` bytes remain.
    pub fn take_raw(&mut self, n: usize) -> Result<&'a [u8]> {
        let start = self.pos;
        self.skip(n)?;
        Ok(&self.input[start..self.pos])
    }

    /// Reads a 32-bit little-endian unsigned integer.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than four bytes remain.
    pub fn take_u32(&mut self) -> Result<u32> {
        let b = self.take_raw(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a 32-bit little-endian signed integer.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than four bytes remain.
    pub fn take_i32(&mut self) -> Result<i32> {
        Ok(self.take_u32()? as i32)
    }

    /// Reads a 64-bit little-endian unsigned integer.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than eight bytes remain.
    pub fn take_u64(&mut self) -> Result<u64> {
        let b = self.take_raw(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a 64-bit little-endian signed integer.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than eight bytes remain.
    pub fn take_i64(&mut self) -> Result<i64> {
        Ok(self.take_u64()? as i64)
    }

    /// Reads a 64-bit little-endian IEEE-754 double.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than eight bytes remain.
    pub fn take_f64(&mut self) -> Result<f64> {
        let b = self.take_raw(8)?;
        Ok(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a TL `bool` and returns its value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedId`] if the constructor is neither
    /// `boolTrue` nor `boolFalse`.
    pub fn take_bool(&mut self) -> Result<bool> {
        match self.take_u32()? {
            TYPE_BOOL_TRUE => Ok(true),
            TYPE_BOOL_FALSE => Ok(false),
            id => Err(Error::UnexpectedId {
                id,
                expected: TYPE_BOOL_TRUE,
            }),
        }
    }

    /// Reads a constructor identifier without advancing the cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than four bytes remain.
    pub fn peek_id(&self) -> Result<u32> {
        if self.remaining() < 4 {
            return Err(Error::UnexpectedEof {
                needed: 4,
                remaining: self.remaining(),
            });
        }
        let b = &self.input[self.pos..self.pos + 4];
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a constructor identifier and errors unless it equals `expected`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedId`] when the identifier differs.
    pub fn consume_id(&mut self, expected: u32) -> Result<()> {
        let id = self.take_u32()?;
        if id == expected {
            Ok(())
        } else {
            Err(Error::UnexpectedId { id, expected })
        }
    }

    /// Reads a TL length prefix (1 or 4 bytes) and returns the payload length
    /// together with the number of prefix bytes consumed.
    fn take_len(&mut self) -> Result<(usize, usize)> {
        let first = self.take_raw(1)?[0];
        if first < FIRST_LONG_STRING {
            Ok((first as usize, 1))
        } else if first == FIRST_LONG_STRING {
            let b = self.take_raw(3)?;
            let len = (b[0] as usize) | ((b[1] as usize) << 8) | ((b[2] as usize) << 16);
            Ok((len, 4))
        } else {
            Err(Error::InvalidLength {
                length: first as usize,
                context: "length prefix",
            })
        }
    }

    /// Reads a TL byte string, borrowing from the input.
    ///
    /// # Errors
    ///
    /// Returns an error if the length prefix is invalid or the payload does not
    /// fit in the remaining input.
    pub fn take_bytes(&mut self) -> Result<&'a [u8]> {
        let (len, prefix) = self.take_len()?;
        let data = self.take_raw(len)?;
        self.skip(padding_for(prefix + len))?;
        Ok(data)
    }

    /// Reads a TL string, borrowing from the input and validating UTF-8.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUtf8`] if the payload is not valid UTF-8, or a
    /// length/EOF error as for [`Reader::take_bytes`].
    pub fn take_string(&mut self) -> Result<&'a str> {
        let bytes = self.take_bytes()?;
        core::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
    }

    /// Reads a 128-bit TL integer.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than sixteen bytes remain.
    pub fn take_int128(&mut self) -> Result<Int128> {
        let b = self.take_raw(Int128::LEN)?;
        let mut arr = [0u8; Int128::LEN];
        arr.copy_from_slice(b);
        Ok(Int128::from_array(arr))
    }

    /// Reads a 256-bit TL integer.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than thirty-two bytes remain.
    pub fn take_int256(&mut self) -> Result<Int256> {
        let b = self.take_raw(Int256::LEN)?;
        let mut arr = [0u8; Int256::LEN];
        arr.copy_from_slice(b);
        Ok(Int256::from_array(arr))
    }

    /// Reads a TL vector header and returns the element count.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedId`] if the constructor is not `vector`, or
    /// [`Error::NegativeVectorCount`] if the count is negative.
    pub fn take_vector_header(&mut self) -> Result<usize> {
        self.consume_id(TYPE_VECTOR)?;
        let n = self.take_i32()?;
        if n < 0 {
            return Err(Error::NegativeVectorCount(n));
        }
        Ok(n as usize)
    }
}

/// The largest payload accepted by [`Reader::take_bytes`] is bounded by the
/// three-byte length form ([`crate::types::MAX_LONG_STRING`]); this constant is
/// re-exported here for documentation purposes.
pub const MAX_PAYLOAD_LEN: usize = crate::types::MAX_LONG_STRING;

#[cfg(test)]
mod tests {
    use super::Reader;
    use crate::error::Error;
    use crate::types::{TYPE_VECTOR, WORD};

    #[test]
    fn reads_integers() {
        let mut r = Reader::new(&[1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(r.take_u32().unwrap(), 1);
        assert_eq!(r.take_u64().unwrap(), 1);
        assert!(r.is_empty());
    }

    #[test]
    fn eof_is_reported() {
        let mut r = Reader::new(&[1, 2]);
        let err = r.take_u32().unwrap_err();
        assert_eq!(
            err,
            Error::UnexpectedEof {
                needed: 4,
                remaining: 2
            }
        );
    }

    #[test]
    fn skip_and_seek() {
        let mut r = Reader::new(&[0, 1, 2, 3, 4]);
        r.skip(2).unwrap();
        assert_eq!(r.position(), 2);
        assert_eq!(r.take_raw(1).unwrap(), &[2]);
        r.seek(0).unwrap();
        assert_eq!(r.peek_rest(), &[0, 1, 2, 3, 4]);
        r.reset();
        assert_eq!(r.position(), 0);
        assert!(r.seek(99).is_err());
    }

    #[test]
    fn string_round_trip_borrows_input() {
        // "abc" -> 03 61 62 63
        let input = [3u8, b'a', b'b', b'c'];
        let mut r = Reader::new(&input);
        assert_eq!(r.take_string().unwrap(), "abc");
    }

    #[test]
    fn negative_vector_count_is_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&TYPE_VECTOR.to_le_bytes());
        buf.extend_from_slice(&(-1i32).to_le_bytes());
        let mut r = Reader::new(&buf);
        assert_eq!(
            r.take_vector_header().unwrap_err(),
            Error::NegativeVectorCount(-1)
        );
    }

    #[test]
    fn invalid_length_marker_is_rejected() {
        let input = [255u8, 0, 0, 0];
        let mut r = Reader::new(&input);
        assert!(matches!(
            r.take_bytes().unwrap_err(),
            Error::InvalidLength { .. }
        ));
    }

    #[test]
    fn word_constant_is_used_by_padding() {
        assert_eq!(crate::types::padding_for(4), 0);
        assert_eq!(crate::types::padding_for(5), 3);
        assert_eq!(WORD, 4);
    }
}
