//! Plaintext frame transport and message identifiers.

use rtd_proto::{MessageId, MessageType, UnencryptedMessage, new_message_id};
use rtd_util::Clock;

use crate::error::{Error, Result};

/// A transport that moves plaintext MTProto frames.
///
/// The frame passed to [`Stream::send`] and returned by [`Stream::recv`] is the
/// encoded [`UnencryptedMessage`] (its `auth_key_id` is zero). The transport
/// owns the codec, so this trait is independent of how frames are framed.
pub trait Stream {
    /// Sends one encoded plaintext frame.
    ///
    /// # Errors
    ///
    /// Returns an error when the frame cannot be written.
    fn send(&mut self, frame: &[u8]) -> Result<()>;

    /// Receives one encoded plaintext frame.
    ///
    /// # Errors
    ///
    /// Returns an error when the frame cannot be read.
    fn recv(&mut self) -> Result<Vec<u8>>;
}

/// A monotonic message-id generator for one connection.
pub(crate) struct MessageIds {
    nano: i64,
}

impl MessageIds {
    /// Creates a generator positioned before the epoch.
    pub(crate) const fn new() -> Self {
        Self { nano: 0 }
    }

    /// Returns the next identifier for `ty`, advancing at least ten nanoseconds
    /// past the previous one to avoid collisions on a coarse clock.
    pub(crate) fn next(&mut self, clock: &dyn Clock, ty: MessageType) -> i64 {
        const MIN_RESOLUTION_NANOS: i64 = 10;
        let now = clock.unix_nanos();
        if now > self.nano {
            self.nano = now;
        } else {
            self.nano += MIN_RESOLUTION_NANOS;
        }
        new_message_id(self.nano, ty).as_i64()
    }
}

/// Encodes `payload` as a plaintext message of `ty` and sends it.
pub(crate) fn write_unencrypted(
    stream: &mut impl Stream,
    ids: &mut MessageIds,
    clock: &dyn Clock,
    ty: MessageType,
    payload: &[u8],
) -> Result<()> {
    let message_id = ids.next(clock, ty);
    let frame = UnencryptedMessage::new(message_id, payload.to_vec())
        .encode()
        .map_err(|error| Error::Decode(error.to_string()))?;
    stream.send(&frame)
}

/// Reads one plaintext message and returns its payload, requiring type `ty`.
pub(crate) fn read_unencrypted(stream: &mut impl Stream, ty: MessageType) -> Result<Vec<u8>> {
    let frame = stream.recv()?;
    decode_unencrypted(&frame, ty)
}

/// Decodes a plaintext message of the expected type.
pub(crate) fn decode_unencrypted(frame: &[u8], ty: MessageType) -> Result<Vec<u8>> {
    let message =
        UnencryptedMessage::decode(frame).map_err(|error| Error::Decode(error.to_string()))?;
    let actual = MessageId::new(message.message_id).message_type();
    if actual != ty {
        return Err(Error::Decode(format!(
            "unexpected message type: expected {ty}, got {actual}"
        )));
    }
    Ok(message.data)
}

/// Returns the leading `auth_key_id` of a raw frame, or zero when it is a
/// plaintext frame.
pub(crate) fn frame_auth_key_id(frame: &[u8]) -> Result<i64> {
    let bytes: [u8; 8] = frame
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| Error::Decode("frame shorter than an auth key id".to_owned()))?;
    Ok(i64::from_le_bytes(bytes))
}
