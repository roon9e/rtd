//! MTProto message framing and transport codecs.
//!
//! This crate implements the server-facing protocol core: message identifiers
//! and replay mitigation, message containers, packed (gzipped) payloads, RPC
//! result framing, plaintext messages, and the abridged, intermediate,
//! padded-intermediate and full transport codecs.
//!
//! Wire constants and byte layout match the reference `td` implementation
//! exactly. All multi-byte integers on the wire are little-endian.

pub mod codec;
pub mod container;
pub mod error;
pub mod gzip;
pub mod message_id;
pub mod rpc_result;
pub mod unencrypted;

pub use codec::{
    ABRIDGED_HEADER, Abridged, CODE_AUTH_KEY_NOT_FOUND, CODE_TRANSPORT_FLOOD, CODE_WRONG_DC, Codec,
    CodecError, CodecKind, Full, INTERMEDIATE_HEADER, Intermediate, MAX_MESSAGE_SIZE, NoHeader,
    PADDED_INTERMEDIATE_HEADER, PaddedIntermediate, ProtocolError, detect_codec,
};
pub use container::{
    ContainerMessage, MAX_MESSAGE_BODY, MESSAGE_CONTAINER_TYPE_ID, MessageContainer,
};
pub use error::{Error, Result};
pub use gzip::{GZIP_TYPE_ID, Gzip, MAX_DECOMPRESSED_SIZE};
pub use message_id::{
    MessageId, MessageIdBuf, MessageIdGen, MessageType, new_message_id, new_message_id_time,
};
pub use rpc_result::{RPC_RESULT_TYPE_ID, RpcResult};
pub use unencrypted::UnencryptedMessage;
