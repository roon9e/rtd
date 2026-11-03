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

/// A server salt with the window in which it is valid.
///
/// Mirrors MTProto's `future_salt`: `valid_since` is when the salt becomes
/// usable and `valid_until` when it expires (`0` means "no expiry").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FutureSalt {
    valid_since: i64,
    valid_until: i64,
    salt: i64,
}

/// A clock-driven store of server salts, refreshed by `get_future_salts`.
///
/// The active salt is the one whose validity window contains the supplied time
/// and which became valid most recently; entries whose window has passed are
/// dropped. Selection is a pure function of the injected time, so it is
/// deterministic in tests.
#[derive(Debug, Clone)]
pub struct SaltStore {
    active: i64,
    future: Vec<FutureSalt>,
}

impl SaltStore {
    /// Creates a store whose active salt is `salt` with no expiry.
    #[must_use]
    pub fn new(salt: i64) -> Self {
        Self {
            active: salt,
            future: Vec::new(),
        }
    }

    /// Returns the active salt.
    #[must_use]
    pub fn active(&self) -> i64 {
        self.active
    }

    /// Records a future salt with its validity window.
    pub fn insert(&mut self, valid_since: i64, valid_until: i64, salt: i64) {
        self.future.push(FutureSalt {
            valid_since,
            valid_until,
            salt,
        });
    }

    /// Advances the active salt to the newest one valid at `now`, pruning
    /// expired entries. Returns whether the active salt changed.
    pub fn advance(&mut self, now: i64) -> bool {
        // Drop salts that can never be selected again.
        self.future
            .retain(|entry| entry.valid_until == 0 || entry.valid_until > now);
        let candidate = self
            .future
            .iter()
            .filter(|entry| entry.valid_since <= now)
            .max_by_key(|entry| entry.valid_since)
            .map(|entry| entry.salt);
        match candidate {
            Some(salt) if salt != self.active => {
                self.active = salt;
                true
            }
            _ => false,
        }
    }

    /// Resets the active salt, discarding any future salts.
    pub fn reset(&mut self, salt: i64) {
        self.active = salt;
        self.future.clear();
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
    /// Server salts, refreshed by `get_future_salts`.
    salts: SaltStore,
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
            salts: SaltStore::new(salt),
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

    /// Returns the active server salt.
    #[must_use]
    pub fn salt(&self) -> i64 {
        self.salts.active()
    }

    /// Adopts a new server salt, discarding any future salts.
    pub fn set_salt(&mut self, salt: i64) {
        self.salts.reset(salt);
        self.session.set_salt(salt);
    }

    /// Records a future salt from a `future_salts` reply.
    pub fn insert_future_salt(&mut self, valid_since: i64, valid_until: i64, salt: i64) {
        self.salts.insert(valid_since, valid_until, salt);
    }

    /// Advances the active salt to the newest one valid at `now`.
    ///
    /// Returns whether the active salt changed; the caller then uses
    /// [`ClientSession::salt`] for subsequent messages.
    pub fn advance_salts(&mut self, now: i64) -> bool {
        let changed = self.salts.advance(now);
        if changed {
            self.session.set_salt(self.salts.active());
        }
        changed
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

    #[test]
    fn future_salts_advance_by_clock() {
        let mut session = session();
        assert_eq!(session.salt(), -99);

        session.insert_future_salt(100, 200, 111);
        session.insert_future_salt(150, 300, 222);

        // Before any window opens, the active salt is unchanged.
        assert!(!session.advance_salts(50));
        assert_eq!(session.salt(), -99);

        // The first salt becomes valid.
        assert!(session.advance_salts(120));
        assert_eq!(session.salt(), 111);

        // The newest valid salt wins.
        assert!(session.advance_salts(160));
        assert_eq!(session.salt(), 222);

        // Expired salts never replace the active one.
        assert!(!session.advance_salts(400));
        assert_eq!(session.salt(), 222);
    }

    #[test]
    fn set_salt_discards_future_salts() {
        let mut session = session();
        session.insert_future_salt(1, 2, 5);
        session.set_salt(7);
        assert_eq!(session.salt(), 7);
        assert!(!session.advance_salts(10));
        assert_eq!(session.salt(), 7);
    }
}
