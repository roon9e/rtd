//! Legacy MTProto 1.0 key derivation.
//!
//! MTProto 1.0 is deprecated, but Telegram still requires its exact key schedule
//! for the `auth.bindTempAuthKey` envelope, which always uses the client-side
//! (`x = 0`) derivation regardless of the transport's actual version.

use crate::key::Key;
use rtd_bin::Int128;
use sha1::{Digest, Sha1};

/// Computes `msg_key` for MTProto 1.0: `SHA1(plaintext)[4..20]`.
#[must_use]
pub fn message_key_v1(plaintext: &[u8]) -> Int128 {
    let digest = Sha1::digest(plaintext);
    let mut msg_key = [0u8; 16];
    msg_key.copy_from_slice(&digest[4..20]);
    Int128::from_array(msg_key)
}

/// Derives the legacy `(aes_key, aes_iv)` pair.
///
/// This mirrors the old client-side key schedule with `x = 0`:
///
/// ```text
/// sha1_a = SHA1(msg_key ++ auth_key[0..32])
/// sha1_b = SHA1(auth_key[32..48] ++ msg_key ++ auth_key[48..64])
/// sha1_c = SHA1(auth_key[64..96] ++ msg_key)
/// sha1_d = SHA1(msg_key ++ auth_key[96..128])
/// aes_key = a[0..8] ++ b[8..20] ++ c[4..16]
/// aes_iv  = a[8..20] ++ b[0..8] ++ c[16..20] ++ d[0..8]
/// ```
#[must_use]
pub fn keys_v1(auth_key: &Key, msg_key: &Int128) -> ([u8; 32], [u8; 32]) {
    let auth = auth_key.as_bytes();
    let msg = msg_key.as_array();

    let mut hasher = Sha1::new();
    hasher.update(msg);
    hasher.update(&auth[0..32]);
    let a = hasher.finalize();

    let mut hasher = Sha1::new();
    hasher.update(&auth[32..48]);
    hasher.update(msg);
    hasher.update(&auth[48..64]);
    let b = hasher.finalize();

    let mut hasher = Sha1::new();
    hasher.update(&auth[64..96]);
    hasher.update(msg);
    let c = hasher.finalize();

    let mut hasher = Sha1::new();
    hasher.update(msg);
    hasher.update(&auth[96..128]);
    let d = hasher.finalize();

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&a[0..8]);
    key[8..20].copy_from_slice(&b[8..20]);
    key[20..32].copy_from_slice(&c[4..16]);

    let mut iv = [0u8; 32];
    iv[0..12].copy_from_slice(&a[8..20]);
    iv[12..20].copy_from_slice(&b[0..8]);
    iv[20..24].copy_from_slice(&c[16..20]);
    iv[24..32].copy_from_slice(&d[0..8]);

    (key, iv)
}

#[cfg(test)]
mod tests {
    use super::{keys_v1, message_key_v1};
    use crate::key::Key;
    use rtd_bin::Int128;

    #[test]
    fn known_message_key_v1() {
        let msg_key = message_key_v1(b"bind-message-test-payload");
        let expected: [u8; 16] = hex::decode("fc11a3669566de22b87f66550dc3de6d")
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(msg_key.as_array(), &expected);
    }

    #[test]
    fn known_keys_v1() {
        let mut auth_bytes = [0u8; 256];
        for (i, byte) in auth_bytes.iter_mut().enumerate() {
            *byte = i as u8;
        }
        let auth = Key::from_array(auth_bytes);
        let mut msg_bytes = [0u8; 16];
        for (i, byte) in msg_bytes.iter_mut().enumerate() {
            *byte = i as u8;
        }
        let msg_key = Int128::from_array(msg_bytes);

        let (key, iv) = keys_v1(&auth, &msg_key);
        assert_eq!(
            key,
            hex_array("17d7295ca9213d1ab656acdb1ad48b2ea7f3a8f7095098d5508b900bbd5fccfc")
        );
        assert_eq!(
            iv,
            hex_array("2d7d16a65a84108e9805656caa474501cc580aa2edc33abfd0bfad785464d1c6")
        );
    }

    fn hex_array<const N: usize>(hex: &str) -> [u8; N] {
        hex::decode(hex)
            .expect("valid hex")
            .try_into()
            .expect("correct length")
    }
}
