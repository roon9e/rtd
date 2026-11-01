//! The MTProto 2.0 encrypted-message envelope.
//!
//! An encrypted message is framed on the wire as
//!
//! ```text
//! auth_key_id(8) ++ msg_key(16) ++ AES-IGE(plaintext)
//! ```
//!
//! where the plaintext is the inner header, the body and random padding:
//!
//! ```text
//! salt(8) ++ session_id(8) ++ msg_id(8) ++ seq_no(4) ++ body_len(4)
//!     ++ body(body_len) ++ padding(...)
//! ```
//!
//! The total plaintext length is always a multiple of the 16-byte AES block
//! size, and at least [`MIN_PADDING`] bytes of the payload are random padding.
//! The padding is fully random; the message size is deliberately *not* rounded
//! to fixed buckets here (the server does not need that hardening).
//!
//! `msg_key` is `SHA256(auth_key[88 + x .. 120 + x] ++ plaintext)[8..24]`,
//! where `x` is the side-dependent offset (0 for the client, 8 for the server).
//! The AES key and IGE initialization vector are derived from `msg_key` by
//! [`crate::kdf`]. Because the receiver can recompute `msg_key` from the
//! decrypted plaintext, an altered ciphertext is detected as a mismatch.
//!
//! A zero `auth_key_id` marks a *plaintext* MTProto message (see
//! [`crate::bind`] for the only encrypted exception). Such a frame is not an
//! encrypted message and is rejected by [`EncryptedMessage::decode`] and
//! [`decrypt`] with [`Error::PlaintextMessage`]; use [`auth_key_id`] to read
//! the identifier before choosing the plaintext path.
//!
//! See <https://core.telegram.org/mtproto/description>.

use crate::error::{Error, Result};
use crate::ige;
use crate::kdf::{self, Side};
use crate::key::AuthKey;
use crate::rand::RandomSource;
use rtd_bin::Int128;

/// The AES block size in bytes.
const BLOCK: usize = 16;

/// Length of the `auth_key_id` field in bytes.
pub const AUTH_KEY_ID_LEN: usize = 8;
/// Length of the `msg_key` field in bytes.
pub const MSG_KEY_LEN: usize = 16;
/// Length of the outer encrypted-message header (`auth_key_id ++ msg_key`).
pub const ENCRYPTED_HEADER_LEN: usize = AUTH_KEY_ID_LEN + MSG_KEY_LEN;
/// Length of the inner header (`salt ++ session_id ++ msg_id ++ seq_no ++
/// body_len`) in bytes.
pub const INNER_HEADER_LEN: usize = 32;
/// Minimum number of random padding bytes appended to the inner payload.
pub const MIN_PADDING: usize = 12;
/// Maximum accepted padding length in bytes.
pub const MAX_PADDING: usize = 1024;

/// Returns the 8-byte authorization-key identifier at the start of `data`.
///
/// A zero identifier identifies a plaintext message. Returns [`None`] when
/// fewer than eight bytes are available.
#[must_use]
pub fn auth_key_id(data: &[u8]) -> Option<[u8; AUTH_KEY_ID_LEN]> {
    data.get(..AUTH_KEY_ID_LEN)?.try_into().ok()
}

/// The inner header of an MTProto 2.0 encrypted message.
///
/// On the wire the header is followed by the 4-byte body length, the body and
/// random padding; this type carries only the four fields that precede the
/// body length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MessageHeader {
    /// Server salt at the time the message was sent.
    pub salt: i64,
    /// Identifier of the session that owns the message.
    pub session_id: i64,
    /// Unique message identifier.
    pub msg_id: i64,
    /// Sequence number (content and service messages share the space).
    pub seq_no: i32,
}

impl MessageHeader {
    /// The encoded length of the four header fields in bytes.
    pub const ENCODED_LEN: usize = 28;

    /// Encodes the header fields into a new byte vector.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::ENCODED_LEN);
        out.extend_from_slice(&self.salt.to_le_bytes());
        out.extend_from_slice(&self.session_id.to_le_bytes());
        out.extend_from_slice(&self.msg_id.to_le_bytes());
        out.extend_from_slice(&self.seq_no.to_le_bytes());
        out
    }

    /// Decodes the header fields from the start of `data`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MalformedMessage`] if `data` is shorter than
    /// [`MessageHeader::ENCODED_LEN`].
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < Self::ENCODED_LEN {
            return Err(Error::MalformedMessage {
                reason: "inner message header is truncated",
            });
        }
        Ok(Self {
            salt: i64::from_le_bytes(data[0..8].try_into().expect("8 bytes")),
            session_id: i64::from_le_bytes(data[8..16].try_into().expect("8 bytes")),
            msg_id: i64::from_le_bytes(data[16..24].try_into().expect("8 bytes")),
            seq_no: i32::from_le_bytes(data[24..28].try_into().expect("4 bytes")),
        })
    }
}

/// An MTProto 2.0 encrypted message frame.
///
/// The `encrypted_data` payload is a multiple of 16 bytes; its length and the
/// recomputed message key are validated by [`decrypt`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedMessage {
    /// The 8-byte fingerprint of the authorization key.
    pub auth_key_id: [u8; AUTH_KEY_ID_LEN],
    /// The 16-byte message key derived from the plaintext.
    pub msg_key: [u8; MSG_KEY_LEN],
    /// The AES-IGE-encrypted inner payload.
    pub encrypted_data: Vec<u8>,
}

impl EncryptedMessage {
    /// Creates an encrypted message from its decoded fields.
    #[must_use]
    pub fn new(
        auth_key_id: [u8; AUTH_KEY_ID_LEN],
        msg_key: [u8; MSG_KEY_LEN],
        encrypted_data: Vec<u8>,
    ) -> Self {
        Self {
            auth_key_id,
            msg_key,
            encrypted_data,
        }
    }

    /// Decodes a frame from its wire representation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PlaintextMessage`] if the frame carries a zero
    /// authorization-key identifier (it is a plaintext message), or
    /// [`Error::MalformedMessage`] if `data` is shorter than the outer header.
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < ENCRYPTED_HEADER_LEN {
            return Err(Error::MalformedMessage {
                reason: "encrypted message frame is shorter than its header",
            });
        }
        let mut auth_key_id = [0u8; AUTH_KEY_ID_LEN];
        auth_key_id.copy_from_slice(&data[..AUTH_KEY_ID_LEN]);
        if auth_key_id == [0u8; AUTH_KEY_ID_LEN] {
            return Err(Error::PlaintextMessage);
        }
        let mut msg_key = [0u8; MSG_KEY_LEN];
        msg_key.copy_from_slice(&data[AUTH_KEY_ID_LEN..ENCRYPTED_HEADER_LEN]);
        Ok(Self {
            auth_key_id,
            msg_key,
            encrypted_data: data[ENCRYPTED_HEADER_LEN..].to_vec(),
        })
    }

    /// Encodes the frame into its wire representation.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ENCRYPTED_HEADER_LEN + self.encrypted_data.len());
        out.extend_from_slice(&self.auth_key_id);
        out.extend_from_slice(&self.msg_key);
        out.extend_from_slice(&self.encrypted_data);
        out
    }

    /// Reports whether the frame carries a zero authorization-key identifier,
    /// i.e. whether it is a plaintext message rather than an encrypted one.
    #[must_use]
    pub fn is_plaintext(&self) -> bool {
        self.auth_key_id == [0u8; AUTH_KEY_ID_LEN]
    }

    /// Returns the `msg_key` as an [`Int128`].
    #[must_use]
    pub const fn msg_key(&self) -> Int128 {
        Int128::from_array(self.msg_key)
    }
}

/// A decrypted MTProto 2.0 message: its inner header and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptedMessage {
    /// The recovered inner header.
    pub header: MessageHeader,
    /// The recovered message body, without the inner header and padding.
    pub body: Vec<u8>,
}

/// Returns the random padding length for an unpadded inner payload.
///
/// The result brings `unpadded` up to a multiple of the AES block size while
/// never going below [`MIN_PADDING`], so it lies in `MIN_PADDING..=27`.
const fn padding_len(unpadded: usize) -> usize {
    MIN_PADDING + (BLOCK - (unpadded + MIN_PADDING) % BLOCK) % BLOCK
}

/// Compares two byte slices in constant time.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Encrypts a message for `auth_key` with MTProto 2.0.
///
/// `side` is the side performing the encryption (the sender's side): it selects
/// the key-schedule offset. The receiver must pass the same `side`.
///
/// Random padding of at least [`MIN_PADDING`] bytes is drawn from `random`; the
/// encrypted payload length is a multiple of 16.
///
/// # Panics
///
/// Panics if `body` is longer than `u32::MAX` bytes.
#[must_use]
pub fn encrypt(
    auth_key: &AuthKey,
    header: &MessageHeader,
    body: &[u8],
    side: Side,
    random: &mut impl RandomSource,
) -> EncryptedMessage {
    let body_len = u32::try_from(body.len()).expect("message body must fit in 32 bits");
    let unpadded = INNER_HEADER_LEN + body.len();
    let padding = padding_len(unpadded);

    let mut plaintext = Vec::with_capacity(unpadded + padding);
    plaintext.extend_from_slice(&header.encode());
    plaintext.extend_from_slice(&body_len.to_le_bytes());
    plaintext.extend_from_slice(body);
    let pad_start = plaintext.len();
    plaintext.resize(unpadded + padding, 0);
    random.fill(&mut plaintext[pad_start..]);

    let msg_key = kdf::message_key(&auth_key.value, &plaintext, side);
    let (key, iv) = kdf::keys(&auth_key.value, &msg_key, side);
    let mut encrypted_data = plaintext;
    ige::encrypt(&key, &iv, &mut encrypted_data);

    EncryptedMessage {
        auth_key_id: auth_key.id,
        msg_key: *msg_key.as_array(),
        encrypted_data,
    }
}

/// Decrypts and authenticates a message with `auth_key`.
///
/// `side` is the side that produced the message (the sender's side). The
/// message key is recomputed from the decrypted plaintext and compared with the
/// frame's `msg_key`; a mismatch rejects the message with [`Error::HashMismatch`].
///
/// # Errors
///
/// Returns [`Error::PlaintextMessage`] for a zero authorization-key identifier,
/// [`Error::AuthKeyMismatch`] when the frame was encrypted for a different key,
/// [`Error::InvalidBlockLength`] when the payload is not block-aligned,
/// [`Error::HashMismatch`] when the recomputed message key differs, and
/// [`Error::MalformedMessage`] for a truncated payload, an out-of-range body
/// length or invalid padding.
pub fn decrypt(
    auth_key: &AuthKey,
    message: &EncryptedMessage,
    side: Side,
) -> Result<DecryptedMessage> {
    if message.is_plaintext() {
        return Err(Error::PlaintextMessage);
    }
    if message.auth_key_id != auth_key.id {
        return Err(Error::AuthKeyMismatch {
            expected: auth_key.id,
            found: message.auth_key_id,
        });
    }

    let data = &message.encrypted_data;
    if data.len() < INNER_HEADER_LEN + MIN_PADDING {
        return Err(Error::MalformedMessage {
            reason: "encrypted payload is shorter than the minimum message",
        });
    }
    if !data.len().is_multiple_of(BLOCK) {
        return Err(Error::InvalidBlockLength { len: data.len() });
    }

    let msg_key = message.msg_key();
    let (key, iv) = kdf::keys(&auth_key.value, &msg_key, side);
    let mut plaintext = data.clone();
    ige::decrypt(&key, &iv, &mut plaintext);

    let computed = kdf::message_key(&auth_key.value, &plaintext, side);
    if !constant_time_eq(computed.as_array(), &message.msg_key) {
        return Err(Error::HashMismatch);
    }

    let length = i32::from_le_bytes(plaintext[28..32].try_into().expect("4 bytes"));
    if length < 0 {
        return Err(Error::MalformedMessage {
            reason: "body length is negative",
        });
    }
    let body_len = length as usize;
    let max_body = plaintext.len() - INNER_HEADER_LEN;
    if body_len > max_body {
        return Err(Error::MalformedMessage {
            reason: "body length exceeds the decrypted payload",
        });
    }
    let padding = max_body - body_len;
    if !(MIN_PADDING..=MAX_PADDING).contains(&padding) {
        return Err(Error::MalformedMessage {
            reason: "padding length is out of range",
        });
    }

    Ok(DecryptedMessage {
        header: MessageHeader::decode(&plaintext[..MessageHeader::ENCODED_LEN])?,
        body: plaintext[INNER_HEADER_LEN..INNER_HEADER_LEN + body_len].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        AUTH_KEY_ID_LEN, EncryptedMessage, MAX_PADDING, MIN_PADDING, MessageHeader, auth_key_id,
        decrypt, encrypt,
    };
    use crate::error::Error;
    use crate::kdf::Side;
    use crate::key::{AuthKey, Key};
    use crate::testutil::XorShift64;

    fn test_key() -> AuthKey {
        let mut bytes = [0u8; 256];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = i as u8;
        }
        AuthKey::new(Key::from_array(bytes))
    }

    fn header() -> MessageHeader {
        MessageHeader {
            salt: 0x1122_3344_5566_7788,
            session_id: -42,
            msg_id: 0x51e5_7ac4_2770_964a,
            seq_no: 7,
        }
    }

    #[test]
    fn round_trip_recovers_header_and_body() {
        let key = test_key();
        let body = b"0123456789abcdef0123456789abcdef".to_vec();
        for side in [Side::Client, Side::Server] {
            let encrypted = encrypt(&key, &header(), &body, side, &mut XorShift64::new(1));
            assert_eq!(&encrypted.auth_key_id, &key.id);
            assert_eq!(encrypted.encrypted_data.len() % 16, 0);
            let padding = encrypted.encrypted_data.len() - 32 - body.len();
            assert!((MIN_PADDING..=MAX_PADDING).contains(&padding));

            let decrypted = decrypt(&key, &encrypted, side).unwrap();
            assert_eq!(decrypted.header, header());
            assert_eq!(decrypted.body, body);
        }
    }

    #[test]
    fn body_round_trips_through_bytes() {
        let key = test_key();
        let body = b"a short body".to_vec();
        let encrypted = encrypt(
            &key,
            &header(),
            &body,
            Side::Client,
            &mut XorShift64::new(9),
        );
        let wire = encrypted.encode();
        assert_eq!(wire.len(), 24 + encrypted.encrypted_data.len());
        let decoded = EncryptedMessage::decode(&wire).unwrap();
        assert_eq!(decoded, encrypted);
        assert_eq!(auth_key_id(&wire), Some(key.id));
        let decrypted = decrypt(&key, &decoded, Side::Client).unwrap();
        assert_eq!(decrypted.body, body);
    }

    #[test]
    fn empty_body_round_trips() {
        let key = test_key();
        let encrypted = encrypt(&key, &header(), &[], Side::Server, &mut XorShift64::new(3));
        let decrypted = decrypt(&key, &encrypted, Side::Server).unwrap();
        assert_eq!(decrypted.header, header());
        assert!(decrypted.body.is_empty());
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let key = test_key();
        let body = b"tamper target body".to_vec();
        let encrypted = encrypt(
            &key,
            &header(),
            &body,
            Side::Client,
            &mut XorShift64::new(5),
        );

        // Alter a byte of the ciphertext.
        let mut tampered = encrypted.clone();
        let last = tampered.encrypted_data.len() - 1;
        tampered.encrypted_data[last] ^= 0x01;
        assert_eq!(
            decrypt(&key, &tampered, Side::Client),
            Err(Error::HashMismatch)
        );

        // Alter the message key itself.
        let mut key_tampered = encrypted.clone();
        key_tampered.msg_key[0] ^= 0x80;
        assert!(decrypt(&key, &key_tampered, Side::Client).is_err());
    }

    #[test]
    fn wrong_side_or_key_is_rejected() {
        let key = test_key();
        let encrypted = encrypt(
            &key,
            &header(),
            &[0u8; 16],
            Side::Client,
            &mut XorShift64::new(7),
        );
        assert_eq!(
            decrypt(&key, &encrypted, Side::Server),
            Err(Error::HashMismatch)
        );

        let mut other = [0u8; 256];
        other[0] = 1;
        let other = AuthKey::new(Key::from_array(other));
        assert!(matches!(
            decrypt(&other, &encrypted, Side::Client),
            Err(Error::AuthKeyMismatch { .. })
        ));
    }

    #[test]
    fn zero_auth_key_id_is_plaintext() {
        // A raw frame with a zero authorization-key identifier.
        let mut wire = vec![0u8; 24];
        wire.extend_from_slice(&[0u8; 16]);
        assert_eq!(auth_key_id(&wire), Some([0u8; AUTH_KEY_ID_LEN]));
        assert_eq!(
            EncryptedMessage::decode(&wire),
            Err(Error::PlaintextMessage)
        );

        // The same frame constructed directly is also rejected by decrypt.
        let frame = EncryptedMessage::new([0u8; AUTH_KEY_ID_LEN], [0u8; 16], vec![0u8; 16]);
        assert!(frame.is_plaintext());
        assert_eq!(
            decrypt(&test_key(), &frame, Side::Client),
            Err(Error::PlaintextMessage)
        );
    }

    #[test]
    fn malformed_frames_are_rejected() {
        let key = test_key();
        assert!(matches!(
            EncryptedMessage::decode(&[0u8; 10]),
            Err(Error::MalformedMessage { .. })
        ));

        // Non-block-aligned ciphertext.
        let frame = EncryptedMessage::new(key.id, [0u8; 16], vec![0u8; 49]);
        assert!(matches!(
            decrypt(&key, &frame, Side::Client),
            Err(Error::MalformedMessage { .. }) | Err(Error::InvalidBlockLength { .. })
        ));

        // Too short to hold even the inner header and minimum padding.
        let frame = EncryptedMessage::new(key.id, [0u8; 16], vec![0u8; 32]);
        assert!(matches!(
            decrypt(&key, &frame, Side::Client),
            Err(Error::MalformedMessage { .. })
        ));
    }

    #[test]
    fn header_encode_decode_round_trip() {
        let encoded = header().encode();
        assert_eq!(encoded.len(), MessageHeader::ENCODED_LEN);
        assert_eq!(MessageHeader::decode(&encoded).unwrap(), header());
        assert!(MessageHeader::decode(&encoded[..20]).is_err());
    }

    #[test]
    fn padding_is_random_not_zero_or_bucketed() {
        // Two encrypts of the same body differ only in padding/keys and both
        // stay within the minimum alignment window.
        let key = test_key();
        let body = b"padding probe".to_vec();
        let a = encrypt(
            &key,
            &header(),
            &body,
            Side::Client,
            &mut XorShift64::new(1),
        );
        let b = encrypt(
            &key,
            &header(),
            &body,
            Side::Client,
            &mut XorShift64::new(2),
        );
        assert_ne!(a.encrypted_data, b.encrypted_data);
        for message in [&a, &b] {
            let padding = message.encrypted_data.len() - 32 - body.len();
            assert!((MIN_PADDING..=27).contains(&padding), "{padding}");
        }
    }
}
