//! Unencrypted (plaintext) MTProto messages.
//!
//! Plaintext messages are used before an auth key is established. Their
//! `auth_key_id` field is always zero.

use crate::error::{Error, Result};
use rtd_bin::{Reader, Writer};

/// A plaintext MTProto message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnencryptedMessage {
    /// Message identifier.
    pub message_id: i64,
    /// Raw unencrypted payload.
    pub data: Vec<u8>,
}

impl UnencryptedMessage {
    /// Creates a plaintext message from its parts.
    #[must_use]
    pub const fn new(message_id: i64, data: Vec<u8>) -> Self {
        Self { message_id, data }
    }

    /// Encodes the message into a fresh buffer.
    ///
    /// The wire form is a zero `auth_key_id`, the message id, a 32-bit length,
    /// then the raw data.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload does not fit in an `i32` length field.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let len = i32::try_from(self.data.len()).map_err(|_| Error::InvalidLength {
            length: self.data.len() as i64,
            context: "plaintext message data",
        })?;
        let mut writer = Writer::new();
        writer.put_i64(0);
        writer.put_i64(self.message_id);
        writer.put_i32(len);
        writer.put_raw(&self.data);
        Ok(writer.into_vec())
    }

    /// Decodes a plaintext message from a borrowed slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the auth-key id is non-zero, the declared length is
    /// negative or exceeds the remaining input, or the buffer is truncated.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input);
        let auth_key_id = reader.take_i64()?;
        if auth_key_id != 0 {
            return Err(Error::UnexpectedAuthKeyId(auth_key_id));
        }
        let message_id = reader.take_i64()?;
        let len = reader.take_i32()?;
        if len < 0 {
            return Err(Error::InvalidLength {
                length: i64::from(len),
                context: "plaintext message data",
            });
        }
        let len = len as usize;
        if len > reader.remaining() {
            return Err(rtd_bin::Error::UnexpectedEof {
                needed: len,
                remaining: reader.remaining(),
            }
            .into());
        }
        let data = reader.take_raw(len)?.to_vec();
        Ok(Self { message_id, data })
    }

    /// Decodes a plaintext message from a borrowed slice.
    ///
    /// # Errors
    ///
    /// See [`UnencryptedMessage::decode`].
    pub fn from_bytes(input: &[u8]) -> Result<Self> {
        Self::decode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::UnencryptedMessage;
    use crate::error::Error;

    #[test]
    fn round_trip() {
        let original = UnencryptedMessage::new(0x1122_3344_5566_7788, vec![1, 2, 3]);
        let encoded = original.encode().unwrap();
        // auth_key_id must be zero.
        assert_eq!(&encoded[..8], &[0u8; 8]);
        let decoded = UnencryptedMessage::decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn nonzero_auth_key_id_is_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1i64.to_le_bytes());
        buf.extend_from_slice(&0i64.to_le_bytes());
        buf.extend_from_slice(&0i32.to_le_bytes());
        assert!(matches!(
            UnencryptedMessage::decode(&buf),
            Err(Error::UnexpectedAuthKeyId(1))
        ));
    }

    #[test]
    fn negative_length_is_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0i64.to_le_bytes());
        buf.extend_from_slice(&0i64.to_le_bytes());
        buf.extend_from_slice(&(-1i32).to_le_bytes());
        assert!(matches!(
            UnencryptedMessage::decode(&buf),
            Err(Error::InvalidLength { .. })
        ));
    }

    #[test]
    fn length_beyond_remaining_is_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0i64.to_le_bytes());
        buf.extend_from_slice(&0i64.to_le_bytes());
        buf.extend_from_slice(&10i32.to_le_bytes());
        buf.extend_from_slice(&[1, 2, 3]);
        assert!(UnencryptedMessage::decode(&buf).is_err());
    }

    #[test]
    fn from_bytes_matches_decode() {
        let encoded = UnencryptedMessage::new(5, vec![9, 9]).encode().unwrap();
        assert_eq!(
            UnencryptedMessage::from_bytes(&encoded).unwrap(),
            UnencryptedMessage::decode(&encoded).unwrap()
        );
    }
}
