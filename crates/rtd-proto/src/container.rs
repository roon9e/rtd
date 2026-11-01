//! MTProto message containers (`message_container#73f1f8dc`).
//!
//! A container groups several messages so that they can be acknowledged and
//! processed together. Each element carries its own message identifier,
//! sequence number, length and body.

use crate::error::{Error, Result};
use rtd_bin::{Reader, Writer};

/// TL type id of [`MessageContainer`].
pub const MESSAGE_CONTAINER_TYPE_ID: u32 = 0x73f1_f8dc;

/// Maximum accepted body length of a single container element (1 MiB).
pub const MAX_MESSAGE_BODY: usize = 1024 * 1024;

/// A message element inside a [`MessageContainer`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainerMessage {
    /// Message identifier.
    pub id: i64,
    /// MTProto sequence number.
    pub seq_no: i32,
    /// Raw message body.
    pub body: Vec<u8>,
}

impl ContainerMessage {
    /// Creates an element from its parts.
    #[must_use]
    pub fn new(id: i64, seq_no: i32, body: Vec<u8>) -> Self {
        Self { id, seq_no, body }
    }
}

/// A container of several MTProto messages.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageContainer {
    /// Messages carried by this container.
    pub messages: Vec<ContainerMessage>,
}

impl MessageContainer {
    /// Creates an empty container.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Creates a container from the given messages.
    #[must_use]
    pub const fn with_messages(messages: Vec<ContainerMessage>) -> Self {
        Self { messages }
    }

    /// Encodes the container into a fresh buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if any element body exceeds [`MAX_MESSAGE_BODY`] or if
    /// the element count does not fit in an `i32`.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new();
        self.encode_into(&mut writer)?;
        Ok(writer.into_vec())
    }

    /// Encodes the container into `writer`.
    ///
    /// # Errors
    ///
    /// Returns an error if any element body exceeds [`MAX_MESSAGE_BODY`] or if
    /// the element count does not fit in an `i32`.
    pub fn encode_into(&self, writer: &mut Writer) -> Result<()> {
        let count = i32::try_from(self.messages.len()).map_err(|_| Error::InvalidLength {
            length: self.messages.len() as i64,
            context: "container element count",
        })?;
        writer.put_id(MESSAGE_CONTAINER_TYPE_ID);
        writer.put_i32(count);
        for message in &self.messages {
            if message.body.len() > MAX_MESSAGE_BODY {
                return Err(Error::InvalidLength {
                    length: message.body.len() as i64,
                    context: "container message body",
                });
            }
            writer.put_i64(message.id);
            writer.put_i32(message.seq_no);
            writer.put_i32(message.body.len() as i32);
            writer.put_raw(&message.body);
        }
        Ok(())
    }

    /// Decodes a container from `reader`, appending to [`Self::messages`].
    ///
    /// The existing contents are preserved, matching the reference decoder
    /// which appends to the destination slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the type id is wrong, the count is negative, or an
    /// element declares a body length that is negative or above
    /// [`MAX_MESSAGE_BODY`].
    pub fn decode(&mut self, reader: &mut Reader<'_>) -> Result<()> {
        reader.consume_id(MESSAGE_CONTAINER_TYPE_ID)?;
        let count = reader.take_i32()?;
        if count < 0 {
            return Err(Error::InvalidLength {
                length: i64::from(count),
                context: "container element count",
            });
        }
        for _ in 0..count {
            let id = reader.take_i64()?;
            let seq_no = reader.take_i32()?;
            let len = reader.take_i32()?;
            if len < 0 || len as usize > MAX_MESSAGE_BODY {
                return Err(Error::InvalidLength {
                    length: i64::from(len),
                    context: "container message body",
                });
            }
            let body = reader.take_raw(len as usize)?.to_vec();
            self.messages.push(ContainerMessage { id, seq_no, body });
        }
        Ok(())
    }

    /// Decodes a container from a borrowed slice, appending to
    /// [`Self::messages`].
    ///
    /// # Errors
    ///
    /// See [`MessageContainer::decode`].
    pub fn decode_from(&mut self, input: &[u8]) -> Result<()> {
        let mut reader = Reader::new(input);
        self.decode(&mut reader)
    }

    /// Decodes a container from a borrowed slice into a fresh value.
    ///
    /// # Errors
    ///
    /// See [`MessageContainer::decode`].
    pub fn from_bytes(input: &[u8]) -> Result<Self> {
        let mut container = Self::new();
        container.decode_from(input)?;
        Ok(container)
    }
}

#[cfg(test)]
mod tests {
    use super::{ContainerMessage, MAX_MESSAGE_BODY, MESSAGE_CONTAINER_TYPE_ID, MessageContainer};

    #[test]
    fn round_trip() {
        let container = MessageContainer::with_messages(vec![
            ContainerMessage::new(1, 2, vec![1, 2, 3, 4]),
            ContainerMessage::new(5, 6, Vec::new()),
        ]);
        let encoded = container.encode().unwrap();
        assert_eq!(
            u32::from_le_bytes(encoded[..4].try_into().unwrap()),
            MESSAGE_CONTAINER_TYPE_ID
        );
        let decoded = MessageContainer::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, container);
    }

    #[test]
    fn element_length_is_checked_on_encode() {
        let container = MessageContainer::with_messages(vec![ContainerMessage::new(
            1,
            0,
            vec![0u8; MAX_MESSAGE_BODY + 1],
        )]);
        assert!(container.encode().is_err());
    }

    #[test]
    fn oversized_element_is_rejected_on_decode() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&MESSAGE_CONTAINER_TYPE_ID.to_le_bytes());
        buf.extend_from_slice(&1i32.to_le_bytes());
        buf.extend_from_slice(&1i64.to_le_bytes());
        buf.extend_from_slice(&0i32.to_le_bytes());
        buf.extend_from_slice(&((MAX_MESSAGE_BODY + 1) as i32).to_le_bytes());
        assert!(MessageContainer::from_bytes(&buf).is_err());
    }

    #[test]
    fn negative_element_length_is_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&MESSAGE_CONTAINER_TYPE_ID.to_le_bytes());
        buf.extend_from_slice(&1i32.to_le_bytes());
        buf.extend_from_slice(&1i64.to_le_bytes());
        buf.extend_from_slice(&0i32.to_le_bytes());
        buf.extend_from_slice(&(-1i32).to_le_bytes());
        assert!(MessageContainer::from_bytes(&buf).is_err());
    }

    #[test]
    fn decode_appends_to_existing_messages() {
        let first = MessageContainer::with_messages(vec![ContainerMessage::new(1, 0, vec![9])]);
        let encoded = first.encode().unwrap();

        let mut target =
            MessageContainer::with_messages(vec![ContainerMessage::new(2, 0, vec![8])]);
        target.decode_from(&encoded).unwrap();
        assert_eq!(target.messages.len(), 2);
        assert_eq!(target.messages[0].id, 2);
        assert_eq!(target.messages[1].id, 1);
    }

    #[test]
    fn nested_container_decode() {
        let inner =
            MessageContainer::with_messages(vec![ContainerMessage::new(7, 0, vec![0xaa, 0xbb])]);
        let inner_bytes = inner.encode().unwrap();
        let outer =
            MessageContainer::with_messages(vec![ContainerMessage::new(1, 0, inner_bytes.clone())]);
        let outer_bytes = outer.encode().unwrap();

        let decoded_outer = MessageContainer::from_bytes(&outer_bytes).unwrap();
        assert_eq!(decoded_outer.messages.len(), 1);
        let decoded_inner = MessageContainer::from_bytes(&decoded_outer.messages[0].body).unwrap();
        assert_eq!(decoded_inner, inner);
    }
}
