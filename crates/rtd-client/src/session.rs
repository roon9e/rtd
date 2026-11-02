//! The client-side encrypted MTProto session.

use std::sync::Arc;

use rtd_crypto::{
    AuthKey, DecryptedMessage, EncryptedMessage, Key, MessageHeader, Side, decrypt, encrypt,
};
use rtd_proto::{MessageClass, MessageSeq, MessageType, Session};
use rtd_util::Clock;

use crate::error::{Error, Result};

/// Message type the client stamps on every outbound message identifier.
const CLIENT_MESSAGE_TYPE: MessageType = MessageType::FromClient;

/// Magic prefix of a [`SessionData`] blob.
const SNAPSHOT_MAGIC: [u8; 4] = *b"RTDS";
/// Current [`SessionData`] blob version.
const SNAPSHOT_VERSION: u16 = 1;
/// Fixed encoded length of a [`SessionData`] blob.
const SNAPSHOT_LEN: usize = 4 + 2 + Key::LEN + 8 + 8;

/// A serializable snapshot of an established session, for a reconnect that skips
/// the authorization-key exchange.
///
/// It holds the secret authorization key: encode it only into storage the caller
/// trusts, and treat the bytes as confidential.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionData {
    /// The raw 2048-bit authorization key.
    pub auth_key: [u8; Key::LEN],
    /// The session identifier.
    pub session_id: i64,
    /// The current server salt.
    pub salt: i64,
}

impl SessionData {
    /// Encodes the snapshot into a versioned blob.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(SNAPSHOT_LEN);
        out.extend_from_slice(&SNAPSHOT_MAGIC);
        out.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
        out.extend_from_slice(&self.auth_key);
        out.extend_from_slice(&self.session_id.to_le_bytes());
        out.extend_from_slice(&self.salt.to_le_bytes());
        out
    }

    /// Decodes a snapshot, rejecting a foreign, unknown-version or truncated blob.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Decode`] when the blob is not a well-formed snapshot.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SNAPSHOT_LEN {
            return Err(Error::Decode(format!(
                "session snapshot length {} (expected {SNAPSHOT_LEN})",
                bytes.len()
            )));
        }
        if bytes[..4] != SNAPSHOT_MAGIC {
            return Err(Error::Decode("session snapshot magic".to_owned()));
        }
        let version = u16::from_le_bytes(bytes[4..6].try_into().expect("two bytes"));
        if version != SNAPSHOT_VERSION {
            return Err(Error::Decode(format!("session snapshot version {version}")));
        }
        let mut auth_key = [0u8; Key::LEN];
        auth_key.copy_from_slice(&bytes[6..6 + Key::LEN]);
        let session_id = i64::from_le_bytes(
            bytes[6 + Key::LEN..14 + Key::LEN]
                .try_into()
                .expect("eight bytes"),
        );
        let salt = i64::from_le_bytes(
            bytes[14 + Key::LEN..22 + Key::LEN]
                .try_into()
                .expect("eight bytes"),
        );
        Ok(Self {
            auth_key,
            session_id,
            salt,
        })
    }
}

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

    /// Snapshots the durable session state for persistence.
    #[must_use]
    pub fn snapshot(&self) -> SessionData {
        SessionData {
            auth_key: *self.auth_key.value.as_array(),
            session_id: self.session_id(),
            salt: self.salt(),
        }
    }

    /// Rebuilds a session from a snapshot, deriving the key fingerprint.
    #[must_use]
    pub fn from_data(data: &SessionData, clock: Arc<dyn Clock + Send + Sync>) -> Self {
        let auth_key = Key::from_array(data.auth_key).with_id();
        Self::new(auth_key, data.session_id, data.salt, clock)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rtd_crypto::Key;
    use rtd_util::SystemClock;

    use super::{ClientSession, SessionData};

    fn session() -> ClientSession {
        let auth_key = Key::from_array([7u8; Key::LEN]).with_id();
        ClientSession::new(auth_key, 0x1234_5678, -99, Arc::new(SystemClock))
    }

    #[test]
    fn snapshot_round_trips_and_restores() {
        let session = session();
        let data = session.snapshot();
        let decoded = SessionData::decode(&data.encode()).expect("decode");
        assert_eq!(decoded, data);

        let restored = ClientSession::from_data(&decoded, Arc::new(SystemClock));
        assert_eq!(restored.session_id(), session.session_id());
        assert_eq!(restored.salt(), session.salt());
        assert_eq!(
            restored.auth_key().value.as_array(),
            session.auth_key().value.as_array()
        );
        assert_eq!(restored.auth_key().id, session.auth_key().id);
    }

    #[test]
    fn malformed_snapshots_fail_closed() {
        let data = session().snapshot();
        let encoded = data.encode();
        assert!(SessionData::decode(&encoded[..encoded.len() - 1]).is_err());
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 0xff;
        assert!(SessionData::decode(&wrong_magic).is_err());
        let mut wrong_version = encoded.clone();
        wrong_version[4] = 0xff;
        assert!(SessionData::decode(&wrong_version).is_err());
        assert!(SessionData::decode(&[]).is_err());
    }
}
