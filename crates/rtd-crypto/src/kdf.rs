//! MTProto 2.0 key derivation.
//!
//! Implements the key schedule documented at
//! <https://core.telegram.org/mtproto/description#defining-aes-key-and-initialization-vector>.
//!
//! The derived AES-IGE initialization vector is 32 bytes (two AES blocks), as
//! required by [`crate::ige`].

use crate::key::Key;
use rtd_bin::Int128;
use sha2::{Digest, Sha256};

/// The side on which encryption is performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The client side (e.g. messages sent by the client).
    Client,
    /// The server side (e.g. RPC responses).
    Server,
}

impl Side {
    /// Returns the side that decrypts what this side encrypts.
    #[must_use]
    pub const fn decrypt_side(self) -> Self {
        match self {
            Self::Client => Self::Server,
            Self::Server => Self::Client,
        }
    }

    /// Returns the side-dependent offset `x` (0 for the client, 8 for the
    /// server).
    const fn offset(self) -> usize {
        match self {
            Self::Client => 0,
            Self::Server => 8,
        }
    }
}

/// Computes `msg_key` for the given padded plaintext.
///
/// `msg_key_large = SHA256(auth_key[88 + x .. 120 + x] ++ plaintext_padded)`
/// and `msg_key = msg_key_large[8..24]`.
#[must_use]
pub fn message_key(auth_key: &Key, plaintext_padded: &[u8], side: Side) -> Int128 {
    let x = side.offset();
    let mut hasher = Sha256::new();
    hasher.update(&auth_key.as_bytes()[88 + x..88 + x + 32]);
    hasher.update(plaintext_padded);
    let large = hasher.finalize();
    let mut msg_key = [0u8; 16];
    msg_key.copy_from_slice(&large[8..24]);
    Int128::from_array(msg_key)
}

/// Derives the `(aes_key, aes_iv)` pair for AES-256-IGE.
///
/// ```text
/// sha256_a = SHA256(msg_key ++ auth_key[x .. 36 + x])
/// sha256_b = SHA256(auth_key[40 + x .. 76 + x] ++ msg_key)
/// aes_key  = a[0..8]  ++ b[8..24]  ++ a[24..32]
/// aes_iv   = b[0..8]  ++ a[8..24]  ++ b[24..32]
/// ```
#[must_use]
pub fn keys(auth_key: &Key, msg_key: &Int128, side: Side) -> ([u8; 32], [u8; 32]) {
    let x = side.offset();
    let auth = auth_key.as_bytes();
    let msg = msg_key.as_array();

    let mut hasher_a = Sha256::new();
    hasher_a.update(msg);
    hasher_a.update(&auth[x..x + 36]);
    let a = hasher_a.finalize();

    let mut hasher_b = Sha256::new();
    hasher_b.update(&auth[40 + x..40 + x + 36]);
    hasher_b.update(msg);
    let b = hasher_b.finalize();

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&a[0..8]);
    key[8..24].copy_from_slice(&b[8..24]);
    key[24..32].copy_from_slice(&a[24..32]);

    let mut iv = [0u8; 32];
    iv[0..8].copy_from_slice(&b[0..8]);
    iv[8..24].copy_from_slice(&a[8..24]);
    iv[24..32].copy_from_slice(&b[24..32]);

    (key, iv)
}

#[cfg(test)]
mod tests {
    use super::{Side, keys, message_key};
    use crate::ige;
    use crate::key::Key;

    fn sequential_key() -> Key {
        let mut bytes = [0u8; 256];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = i as u8;
        }
        Key::from_array(bytes)
    }

    #[test]
    fn decrypt_side_flips() {
        assert_eq!(Side::Client.decrypt_side(), Side::Server);
        assert_eq!(Side::Server.decrypt_side(), Side::Client);
    }

    #[test]
    fn known_client_and_server_keys() {
        // Independently derived from the MTProto 2.0 key schedule.
        let auth = sequential_key();
        let msg_key = rtd_bin::Int128::from_array([
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ]);

        let (client_key, client_iv) = keys(&auth, &msg_key, Side::Client);
        assert_eq!(
            client_key,
            hex_array("704ed09c8b41668ae8f99d244738f71dbddc44469b6bbd4aa8573dd042bd059e")
        );
        assert_eq!(
            client_iv,
            hex_array("4d266000a550edabbf4c7ce40fd0043cc92230184cd317a5cc9c2482fd3b9318")
        );

        let (server_key, server_iv) = keys(&auth, &msg_key, Side::Server);
        assert_eq!(
            server_key,
            hex_array("217725799b245806458174a1fcfbc883906807b15033fdd0ea2b4d69cf9c364e")
        );
        assert_eq!(
            server_iv,
            hex_array("669a6538917a4fa56ca32360a431c9160be4ad887140980dab91ce7bdc47ffbc")
        );
    }

    #[test]
    fn known_message_keys() {
        let auth = sequential_key();
        let plaintext: Vec<u8> = (0..64u8).collect();
        let client = message_key(&auth, &plaintext, Side::Client);
        let server = message_key(&auth, &plaintext, Side::Server);
        assert_eq!(
            client.as_array(),
            &hex_array::<16>("e49f11289d422637af5d2db54562ad19")
        );
        assert_eq!(
            server.as_array(),
            &hex_array::<16>("aea64dd261353032696868c799753ba6")
        );
    }

    #[test]
    fn client_and_server_interoperate() {
        let auth = sequential_key();
        let plaintext: Vec<u8> = (0..96u8).map(|i| i.wrapping_mul(3)).collect();

        // The two sides derive different key material.
        let probe = message_key(&auth, &plaintext, Side::Client);
        assert_ne!(
            keys(&auth, &probe, Side::Client),
            keys(&auth, &probe, Side::Server)
        );

        // The client encrypts; the server derives the sender's side (it flips
        // its own side) and recovers the exact same AES key/IV.
        let msg_key = message_key(&auth, &plaintext, Side::Client);
        let (key, iv) = keys(&auth, &msg_key, Side::Client);
        let mut encrypted = plaintext.clone();
        ige::encrypt(&key, &iv, &mut encrypted);

        let (dec_key, dec_iv) = keys(&auth, &msg_key, Side::Server.decrypt_side());
        ige::decrypt(&dec_key, &dec_iv, &mut encrypted);
        assert_eq!(encrypted, plaintext);

        // The receiver recomputes the message key with the sender's side.
        assert_eq!(message_key(&auth, &encrypted, Side::Client), msg_key);

        // And the reverse direction works the same way.
        let server_msg_key = message_key(&auth, &plaintext, Side::Server);
        let (server_key, server_iv) = keys(&auth, &server_msg_key, Side::Server);
        let mut response = plaintext.clone();
        ige::encrypt(&server_key, &server_iv, &mut response);
        let (client_key, client_iv) = keys(&auth, &server_msg_key, Side::Client.decrypt_side());
        ige::decrypt(&client_key, &client_iv, &mut response);
        assert_eq!(response, plaintext);
    }

    fn hex_array<const N: usize>(hex: &str) -> [u8; N] {
        hex::decode(hex)
            .expect("valid hex")
            .try_into()
            .expect("correct length")
    }
}
