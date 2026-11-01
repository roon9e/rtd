//! The temporary-auth-key binding payload and its MTProto 1.0 envelope.
//!
//! `auth.bindTempAuthKey` wraps a `bind_auth_key_inner` message in a legacy
//! MTProto 1.0 encrypted-message envelope protected by the *permanent*
//! authorization key, even when regular traffic uses MTProto 2.0.
//!
//! See <https://core.telegram.org/api/pfs#special-binding-message> and
//! <https://core.telegram.org/method/auth.bindTempAuthKey>.

use crate::error::{Error, Result};
use crate::ige;
use crate::kdf_v1::{keys_v1, message_key_v1};
use crate::key::AuthKey;
use crate::rand::RandomSource;
use crate::rand::random_bytes;
use rtd_bin::{Reader, Writer};

/// TL constructor identifier of `bind_auth_key_inner`.
pub const BIND_AUTH_KEY_INNER_TYPE_ID: u32 = 0x75a3_f765;

/// A temporary auth key binding payload.
///
/// ```text
/// bind_auth_key_inner#75a3f765 nonce:long temp_auth_key_id:long
///     perm_auth_key_id:long temp_session_id:long expires_at:int = BindAuthKeyInner
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindAuthKeyInner {
    /// Unique nonce of the binding.
    pub nonce: i64,
    /// Identifier of the temporary authorization key.
    pub temp_auth_key_id: i64,
    /// Identifier of the permanent authorization key.
    pub perm_auth_key_id: i64,
    /// Identifier of the session bound to the temporary key.
    pub temp_session_id: i64,
    /// Unix timestamp at which the temporary key expires.
    pub expires_at: i32,
}

impl BindAuthKeyInner {
    /// Encodes the payload, including its constructor identifier.
    pub fn encode(&self, writer: &mut Writer) {
        writer.put_id(BIND_AUTH_KEY_INNER_TYPE_ID);
        writer.put_i64(self.nonce);
        writer.put_i64(self.temp_auth_key_id);
        writer.put_i64(self.perm_auth_key_id);
        writer.put_i64(self.temp_session_id);
        writer.put_i32(self.expires_at);
    }

    /// Decodes the payload, consuming and validating its constructor identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is truncated or the constructor
    /// identifier does not match [`BIND_AUTH_KEY_INNER_TYPE_ID`].
    pub fn decode(reader: &mut Reader<'_>) -> rtd_bin::Result<Self> {
        reader.consume_id(BIND_AUTH_KEY_INNER_TYPE_ID)?;
        Ok(Self {
            nonce: reader.take_i64()?,
            temp_auth_key_id: reader.take_i64()?,
            perm_auth_key_id: reader.take_i64()?,
            temp_session_id: reader.take_i64()?,
            expires_at: reader.take_i32()?,
        })
    }
}

/// Encrypts a binding message with the permanent key using the MTProto 1.0 KDF.
///
/// The envelope is
/// `random:int128 ++ msg_id:long ++ seq_no:int(0) ++ len:int ++ payload`
/// followed by random padding to a multiple of 16 bytes. `msg_key` is computed
/// from the envelope *before* the padding is appended. The result is
/// `perm_auth_key_id(8) ++ msg_key(16) ++ encrypted_data`.
///
/// # Errors
///
/// Returns [`Error::ZeroAuthKey`] if `perm_key` is the zero key, which cannot be
/// used for encryption.
pub fn encrypt_bind_message(
    random: &mut impl RandomSource,
    perm_key: &AuthKey,
    msg_id: i64,
    inner: &BindAuthKeyInner,
) -> Result<Vec<u8>> {
    if perm_key.is_zero() {
        return Err(Error::ZeroAuthKey);
    }

    let mut payload = Writer::new();
    inner.encode(&mut payload);

    let mut plaintext = Writer::new();
    plaintext.put_raw(&random_bytes(16));
    plaintext.put_i64(msg_id);
    plaintext.put_i32(0);
    plaintext.put_i32(payload.len() as i32);
    plaintext.put_raw(payload.as_slice());

    // The message key covers the envelope without random padding.
    let msg_key = message_key_v1(plaintext.as_slice());

    let remainder = plaintext.len() % 16;
    if remainder != 0 {
        let padding = 16 - remainder;
        let mut bytes = vec![0u8; padding];
        random.fill(&mut bytes);
        plaintext.put_raw(&bytes);
    }

    let (key, iv) = keys_v1(&perm_key.value, &msg_key);
    let mut encrypted = plaintext.into_vec();
    ige::encrypt(&key, &iv, &mut encrypted);

    let mut out = Vec::with_capacity(8 + 16 + encrypted.len());
    out.extend_from_slice(&perm_key.id);
    out.extend_from_slice(msg_key.as_array());
    out.extend_from_slice(&encrypted);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{BIND_AUTH_KEY_INNER_TYPE_ID, BindAuthKeyInner, encrypt_bind_message};
    use crate::ige;
    use crate::kdf_v1::{keys_v1, message_key_v1};
    use crate::key::{AuthKey, Key};
    use crate::testutil::RepeatRandom;
    use rtd_bin::{Reader, Writer};

    fn perm_key() -> AuthKey {
        let mut bytes = [0u8; 256];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = 255 - i as u8;
        }
        AuthKey::new(Key::from_array(bytes))
    }

    #[test]
    fn bind_payload_round_trip() {
        let inner = BindAuthKeyInner {
            nonce: 0x0001_0203_0405_0607,
            temp_auth_key_id: 0x0011_2233_4455_6677,
            perm_auth_key_id: 0x1234_5678_9abc_def0,
            temp_session_id: 0x0102_0304_0506_0708,
            expires_at: 1_735_689_600,
        };

        // Structural round trip through the TL encoder/decoder.
        let mut writer = Writer::new();
        inner.encode(&mut writer);
        assert_eq!(writer.len(), 40);
        let mut reader = Reader::new(writer.as_slice());
        assert_eq!(reader.peek_id().unwrap(), BIND_AUTH_KEY_INNER_TYPE_ID);
        let decoded = BindAuthKeyInner::decode(&mut reader).unwrap();
        assert_eq!(decoded, inner);
    }

    #[test]
    fn encrypt_bind_message_envelope() {
        let perm = perm_key();
        let msg_id = 0x0011_2233_4455_6677;
        let inner = BindAuthKeyInner {
            nonce: 0x0001_0203_0405_0607,
            temp_auth_key_id: 0x0011_2233_4455_6677,
            perm_auth_key_id: perm.int_id(),
            temp_session_id: 0x0010_2030_4050_6070,
            expires_at: 1_735_689_600,
        };

        let encrypted =
            encrypt_bind_message(&mut RepeatRandom { byte: 0xCD }, &perm, msg_id, &inner).unwrap();

        assert_eq!(&encrypted[..8], &perm.id);
        let msg_key_bytes: [u8; 16] = encrypted[8..24].try_into().unwrap();
        let msg_key = rtd_bin::Int128::from_array(msg_key_bytes);
        let data = &encrypted[24..];
        assert_eq!(data.len() % 16, 0);
        assert_eq!(data.len(), 80); // 72-byte envelope + 8 bytes of padding.

        let (key, iv) = keys_v1(&perm.value, &msg_key);
        let mut plaintext = data.to_vec();
        ige::decrypt(&key, &iv, &mut plaintext);

        let envelope_len = 16 + 8 + 4 + 4 + 40;
        assert_eq!(msg_key, message_key_v1(&plaintext[..envelope_len]));

        let mut reader = Reader::new(&plaintext);
        let random_prefix = reader.take_raw(16).unwrap();
        assert_ne!(random_prefix, [0u8; 16]);
        assert_eq!(reader.take_i64().unwrap(), msg_id);
        assert_eq!(reader.take_i32().unwrap(), 0);
        assert_eq!(reader.take_i32().unwrap(), 40);
        let body = reader.take_raw(40).unwrap();
        let mut body_reader = Reader::new(body);
        assert_eq!(BindAuthKeyInner::decode(&mut body_reader).unwrap(), inner);
    }

    #[test]
    fn zero_permanent_key_is_rejected() {
        let zero = AuthKey {
            value: Key::from_array([0u8; 256]),
            id: [0u8; 8],
        };
        let err = encrypt_bind_message(
            &mut RepeatRandom { byte: 0 },
            &zero,
            1,
            &BindAuthKeyInner {
                nonce: 0,
                temp_auth_key_id: 0,
                perm_auth_key_id: 0,
                temp_session_id: 0,
                expires_at: 0,
            },
        )
        .unwrap_err();
        assert!(matches!(err, crate::Error::ZeroAuthKey));
    }
}
