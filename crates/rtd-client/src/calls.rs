//! Voice/video call signaling.
//!
//! Telegram calls carry their `phone.sendSignalingData` payloads through an
//! AES-IGE control channel keyed by the call's shared secret derived during the
//! DH exchange. [`CallControl`] implements that channel; the media path
//! (WebRTC/SFU) is a separate, larger effort.

use rtd_crypto::{ige_decrypt, ige_encrypt};

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
