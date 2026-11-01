//! RPC result framing (`rpc_result#f35c6d01`).

use crate::error::Result;
use rtd_bin::{Reader, Writer};

/// TL type id of [`RpcResult`].
pub const RPC_RESULT_TYPE_ID: u32 = 0xf35c_6d01;

/// An RPC result binding a request message identifier to its result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RpcResult {
    /// Identifier of the request this result answers.
    pub request_message_id: i64,
    /// Raw result payload (may itself be a [`crate::Gzip`] object).
    pub result: Vec<u8>,
}

impl RpcResult {
    /// Creates a result from its parts.
    #[must_use]
    pub const fn new(request_message_id: i64, result: Vec<u8>) -> Self {
        Self {
            request_message_id,
            result,
        }
    }

    /// Encodes the result into a fresh buffer.
    ///
    /// The wire form is the type id, the request message id, then the raw
    /// result bytes with no additional length prefix.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.put_id(RPC_RESULT_TYPE_ID);
        writer.put_i64(self.request_message_id);
        writer.put_raw(&self.result);
        writer.into_vec()
    }

    /// Decodes a result from a borrowed slice.
    ///
    /// After the type id and request message id, the entire remaining buffer is
    /// taken as the result payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the type id is wrong or the buffer is truncated.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input);
        reader.consume_id(RPC_RESULT_TYPE_ID)?;
        let request_message_id = reader.take_i64()?;
        let result = reader.peek_rest().to_vec();
        Ok(Self {
            request_message_id,
            result,
        })
    }

    /// Decodes a result from a borrowed slice.
    ///
    /// # Errors
    ///
    /// See [`RpcResult::decode`].
    pub fn from_bytes(input: &[u8]) -> Result<Self> {
        Self::decode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::{RPC_RESULT_TYPE_ID, RpcResult};

    #[test]
    fn round_trip() {
        let original = RpcResult::new(0x1234_5678_9abc_def0, vec![1, 2, 3, 4, 5]);
        let encoded = original.encode();
        assert_eq!(
            u32::from_le_bytes(encoded[..4].try_into().unwrap()),
            RPC_RESULT_TYPE_ID
        );
        let decoded = RpcResult::decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn result_is_the_entire_remainder() {
        // Build manually: type id + request id + 4-byte payload.
        let mut buf = Vec::new();
        buf.extend_from_slice(&RPC_RESULT_TYPE_ID.to_le_bytes());
        buf.extend_from_slice(&7i64.to_le_bytes());
        buf.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        let decoded = RpcResult::from_bytes(&buf).unwrap();
        assert_eq!(decoded.request_message_id, 7);
        assert_eq!(decoded.result, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn empty_result_is_allowed() {
        let original = RpcResult::new(3, Vec::new());
        assert_eq!(RpcResult::decode(&original.encode()).unwrap(), original);
    }

    #[test]
    fn wrong_type_id_is_rejected() {
        let encoded = RpcResult::new(1, vec![1]).encode();
        let mut corrupted = encoded;
        corrupted[0] ^= 0xff;
        assert!(RpcResult::decode(&corrupted).is_err());
    }
}
