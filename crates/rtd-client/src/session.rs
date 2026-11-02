//! The client-side encrypted MTProto session.

use std::sync::Arc;

use rtd_crypto::{
    AuthKey, DecryptedMessage, EncryptedMessage, MessageHeader, Side, decrypt, encrypt,
};
use rtd_proto::{MessageClass, MessageSeq, MessageType, Session};
use rtd_util::Clock;

use crate::error::{Error, Result};

/// Message type the client stamps on every outbound message identifier.
const CLIENT_MESSAGE_TYPE: MessageType = MessageType::FromClient;

/// The client's encrypted MTProto session: message allocation, encryption and
/// inbound validation on top of the sans-IO [`Session`].
///
/// The inner [`Session`] stays server-provided state (salt, identity); the
/// client uses only its allocation and salt accessors, never its server-biased
/// inbound path.
pub struct ClientSession {
    session: Session,
    auth_key: AuthKey,
    /// Server content-message ids awaiting acknowledgement.
    inbound_acks: Vec<i64>,
}

impl ClientSession {
    /// Adopts an authorization key, session id and server salt.
    #[must_use]
    pub fn new(
        auth_key: AuthKey,
        session_id: i64,
        salt: i64,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        let mut session = Session::with_id(clock, session_id);
        session.set_salt(salt);
        Self {
            session,
            auth_key,
            inbound_acks: Vec::new(),
        }
    }

    /// Returns the authorization key.
    #[must_use]
    pub fn auth_key(&self) -> &AuthKey {
        &self.auth_key
    }

    /// Returns the session identifier.
    #[must_use]
    pub fn session_id(&self) -> i64 {
        self.session.session_id()
    }

    /// Returns the current server salt.
    #[must_use]
    pub fn salt(&self) -> i64 {
        self.session.salt()
    }

    /// Adopts a new server salt.
    pub fn set_salt(&mut self, salt: i64) {
        self.session.set_salt(salt);
    }

    /// Allocates a content message (odd sequence number).
    pub fn allocate_content(&mut self) -> MessageSeq {
        self.session
            .allocate(CLIENT_MESSAGE_TYPE, MessageClass::Content)
    }

    /// Allocates a service message (even sequence number).
    pub fn allocate_service(&mut self) -> MessageSeq {
        self.session
            .allocate(CLIENT_MESSAGE_TYPE, MessageClass::Service)
    }

    /// Encrypts `body` as `message` with MTProto 2.0 and returns the frame.
    pub fn encode(
        &self,
        message: MessageSeq,
        body: &[u8],
        random: &mut impl rtd_crypto::RandomSource,
    ) -> Vec<u8> {
        let header = MessageHeader {
            salt: self.salt(),
            session_id: self.session_id(),
            msg_id: message.msg_id,
            seq_no: message.seq_no,
        };
        encrypt(&self.auth_key, &header, body, Side::Client, random).encode()
    }

    /// Decrypts a server frame and validates the session id.
    ///
    /// Odd-seqno (content) server messages are queued for acknowledgement; drain
    /// them with [`ClientSession::take_acks`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Crypto`] when the frame does not authenticate, and
    /// [`Error::Unexpected`] when it belongs to another session.
    pub fn decode(&mut self, frame: &[u8]) -> Result<DecryptedMessage> {
        let message = EncryptedMessage::decode(frame)?;
        let decrypted = decrypt(&self.auth_key, &message, Side::Server)?;
        if decrypted.header.session_id != self.session_id() {
            return Err(Error::Unexpected(format!(
                "session id {:#018x} does not match {:#018x}",
                decrypted.header.session_id,
                self.session_id()
            )));
        }
        if decrypted.header.seq_no % 2 == 1 {
            self.inbound_acks.push(decrypted.header.msg_id);
        }
        Ok(decrypted)
    }

    /// Drains the server message ids that await acknowledgement.
    pub fn take_acks(&mut self) -> Vec<i64> {
        core::mem::take(&mut self.inbound_acks)
    }
}
