//! Voice/video call signaling.
//!
//! Telegram calls carry their `phone.sendSignalingData` payloads through an
//! AES-IGE control channel keyed by the call's shared secret derived during the
//! DH exchange. [`CallControl`] implements that channel; the media path
//! (WebRTC/SFU) is a separate, larger effort.

use num_bigint::BigUint;
use rtd_crypto::{RandomSource, ige_decrypt, ige_encrypt};
use rtd_tg::{MessagesDhConfig, MessagesGetDhConfig, Method, Object};
use sha1::{Digest as _, Sha1};
use sha2::Sha256;

use crate::client::Client;
use crate::error::Error;
use crate::transport::Transport;

/// The call Diffie-Hellman group size in bytes (2048-bit).
const KEY_SIZE: usize = 256;

/// The Diffie-Hellman group used for a call (`messages.getDhConfig`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhConfig {
    /// The generator `g`.
    pub g: u32,
    /// The 2048-bit prime `p`.
    pub p: BigUint,
}

impl DhConfig {
    /// Validates the group parameters.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when the prime is not 2048-bit or `g` is out of range.
    pub fn validate(&self) -> core::result::Result<(), CallError> {
        if self.p.bits() != (KEY_SIZE * 8) as u64 {
            return Err(CallError("dh prime is not 2048-bit".to_owned()));
        }
        if !(2..=7).contains(&self.g) {
            return Err(CallError(format!("dh generator out of range: {}", self.g)));
        }
        Ok(())
    }

    /// Generates a secret exponent and the public value `g^exp mod p`.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when no valid exponent is found.
    pub fn generate(
        &self,
        random: &mut impl RandomSource,
        server_random: &[u8],
    ) -> core::result::Result<(BigUint, Vec<u8>), CallError> {
        let p_minus_three = &self.p - BigUint::from(3u32);
        let one = BigUint::from(1u32);
        let p_minus_one = &self.p - &one;
        for _ in 0..16 {
            let mut buffer = vec![0u8; KEY_SIZE];
            random.fill(&mut buffer);
            if server_random.len() == buffer.len() {
                for (byte, mask) in buffer.iter_mut().zip(server_random) {
                    *byte ^= mask;
                }
            }
            let exponent = (BigUint::from_bytes_be(&buffer) % &p_minus_three) + BigUint::from(2u32);
            let public = BigUint::from(self.g).modpow(&exponent, &self.p);
            if public > one && public < p_minus_one {
                return Ok((exponent, pad(&public)));
            }
        }
        Err(CallError(
            "failed to generate a valid dh exponent".to_owned(),
        ))
    }

    /// Computes the 256-byte shared key and its fingerprint from a peer public
    /// value and the local exponent.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when the peer public value is out of range.
    pub fn compute_key(
        &self,
        peer_public: &[u8],
        exponent: &BigUint,
    ) -> core::result::Result<(Vec<u8>, i64), CallError> {
        let peer = BigUint::from_bytes_be(peer_public);
        self.check_value(&peer)?;
        let key = pad(&peer.modpow(exponent, &self.p));
        let fingerprint = key_fingerprint(&key);
        Ok((key, fingerprint))
    }

    /// Ensures a public value lies in `(1, p-1)`.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when the value is out of range.
    pub fn check_value(&self, value: &BigUint) -> core::result::Result<(), CallError> {
        if value <= &BigUint::from(1u32) {
            return Err(CallError("dh value too small".to_owned()));
        }
        if value >= &(&self.p - BigUint::from(1u32)) {
            return Err(CallError("dh value too large".to_owned()));
        }
        Ok(())
    }
}

/// Returns `SHA256(g_a)`, the commitment published before `g_b` is revealed.
#[must_use]
pub fn g_a_hash(g_a: &[u8]) -> [u8; 32] {
    Sha256::digest(g_a).into()
}

/// Returns the low 64 bits (little-endian) of `SHA1(key)`.
#[must_use]
pub fn key_fingerprint(key: &[u8]) -> i64 {
    let digest = Sha1::digest(key);
    let tail = &digest[digest.len() - 8..];
    let mut fingerprint = 0i64;
    for (i, byte) in tail.iter().enumerate() {
        fingerprint |= i64::from(*byte) << (i * 8);
    }
    fingerprint
}

/// Fetches the call DH configuration (`messages.getDhConfig`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
/// when the server answers with the not-modified constructor.
pub fn fetch_dh_config<T: Transport>(
    client: &mut Client<T>,
) -> crate::error::Result<(DhConfig, Vec<u8>)> {
    let query = Method::MessagesGetDhConfig(Box::new(MessagesGetDhConfig {
        version: 0,
        random_length: KEY_SIZE as i32,
    }));
    match client.invoke(query)? {
        Object::MessagesDhConfig(config) => match *config {
            MessagesDhConfig::DhConfig { g, p, random, .. } => Ok((
                DhConfig {
                    g: g as u32,
                    p: BigUint::from_bytes_be(&p),
                },
                random,
            )),
            MessagesDhConfig::DhConfigNotModified { .. } => Err(Error::Unexpected(
                "messages.getDhConfig returned not-modified with version 0".to_owned(),
            )),
        },
        other => Err(Error::Unexpected(format!(
            "messages.getDhConfig returned unexpected constructor {:#010x}",
            other.type_id()
        ))),
    }
}

/// The default call protocol parameters (tgcalls v2 layers).
#[must_use]
pub fn default_protocol() -> rtd_tg::PhoneCallProtocol {
    rtd_tg::PhoneCallProtocol {
        flags: 0,
        udp_p2p: true,
        udp_reflector: true,
        min_layer: 92,
        max_layer: 92,
        library_versions: vec!["2.7.7".to_owned()],
    }
}

/// Places a call to `user` (`phone.requestCall`), committing to `g_a_hash`.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
/// when the server answers with a different constructor.
pub fn request_call<T: Transport>(
    client: &mut Client<T>,
    user: rtd_tg::InputUser,
    random_id: i32,
    g_a_hash: Vec<u8>,
    video: bool,
) -> crate::error::Result<rtd_tg::PhonePhoneCall> {
    let query = Method::PhoneRequestCall(Box::new(rtd_tg::PhoneRequestCall {
        flags: if video { 1 } else { 0 },
        video,
        user_id: Box::new(user),
        random_id,
        g_a_hash,
        protocol: Box::new(default_protocol()),
    }));
    expect_phone_call(client.invoke(query)?, "phone.requestCall")
}

/// Accepts an incoming call (`phone.acceptCall`) with `g_b`.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
/// when the server answers with a different constructor.
pub fn accept_call<T: Transport>(
    client: &mut Client<T>,
    peer: rtd_tg::InputPhoneCall,
    g_b: Vec<u8>,
) -> crate::error::Result<rtd_tg::PhonePhoneCall> {
    let query = Method::PhoneAcceptCall(Box::new(rtd_tg::PhoneAcceptCall {
        peer: Box::new(peer),
        g_b,
        protocol: Box::new(default_protocol()),
    }));
    expect_phone_call(client.invoke(query)?, "phone.acceptCall")
}

/// Confirms the shared key of a call (`phone.confirmCall`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
/// when the server answers with a different constructor.
pub fn confirm_call<T: Transport>(
    client: &mut Client<T>,
    peer: rtd_tg::InputPhoneCall,
    g_a: Vec<u8>,
    key_fingerprint: i64,
) -> crate::error::Result<rtd_tg::PhonePhoneCall> {
    let query = Method::PhoneConfirmCall(Box::new(rtd_tg::PhoneConfirmCall {
        peer: Box::new(peer),
        g_a,
        key_fingerprint,
        protocol: Box::new(default_protocol()),
    }));
    expect_phone_call(client.invoke(query)?, "phone.confirmCall")
}

/// Requires a `phone.phoneCall` result.
fn expect_phone_call(object: Object, what: &str) -> crate::error::Result<rtd_tg::PhonePhoneCall> {
    match object {
        Object::PhonePhoneCall(call) => Ok(*call),
        other => Err(Error::Unexpected(format!(
            "{what} returned unexpected constructor {:#010x}",
            other.type_id()
        ))),
    }
}

/// Left-pads the big-endian bytes of `value` to [`KEY_SIZE`].
fn pad(value: &BigUint) -> Vec<u8> {
    let bytes = value.to_bytes_be();
    if bytes.len() >= KEY_SIZE {
        return bytes;
    }
    let mut out = vec![0u8; KEY_SIZE];
    out[KEY_SIZE - bytes.len()..].copy_from_slice(&bytes);
    out
}

/// A call-lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallState {
    /// The call was requested.
    Requested,
    /// The call was accepted.
    Accepted,
    /// The call was discarded.
    Discarded,
}

/// A call signaling error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallError(pub String);

impl core::fmt::Display for CallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for CallError {}

/// The AES-IGE control channel shared by the two call peers.
#[derive(Debug, Clone)]
pub struct CallControl {
    key: [u8; 32],
    iv: [u8; 32],
}

impl CallControl {
    /// Creates a control channel from the 32-byte key and IV.
    #[must_use]
    pub fn new(key: [u8; 32], iv: [u8; 32]) -> Self {
        Self { key, iv }
    }

    /// Encrypts `data`, zero-padding it to the AES block size.
    #[must_use]
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut buffer = data.to_vec();
        let padding = (16 - buffer.len() % 16) % 16;
        buffer.extend(std::iter::repeat_n(0, padding));
        ige_encrypt(&self.key, &self.iv, &mut buffer);
        buffer
    }

    /// Decrypts `data`, which must be AES-block aligned.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when `data` is not a multiple of the AES block size.
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CallError> {
        if !data.len().is_multiple_of(16) {
            return Err(CallError(
                "signaling payload is not block aligned".to_owned(),
            ));
        }
        let mut buffer = data.to_vec();
        ige_decrypt(&self.key, &self.iv, &mut buffer);
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::{CallControl, CallError, CallState};

    #[test]
    fn control_channel_round_trips() {
        let control = CallControl::new([7u8; 32], [9u8; 32]);
        // Aligned input round-trips exactly (padding is the caller's concern).
        let plaintext = [1u8; 32];
        let ciphertext = control.encrypt(&plaintext);
        assert_eq!(ciphertext.len(), 32);
        assert_ne!(ciphertext.as_slice(), plaintext.as_slice());
        assert_eq!(control.decrypt(&ciphertext).unwrap(), plaintext);
    }

    #[test]
    fn rejects_unaligned_ciphertext() {
        let control = CallControl::new([0u8; 32], [0u8; 32]);
        assert!(matches!(control.decrypt(&[0u8; 17]), Err(CallError(_))));
    }

    #[test]
    fn call_states_are_distinct() {
        assert_ne!(CallState::Requested, CallState::Accepted);
    }
}
