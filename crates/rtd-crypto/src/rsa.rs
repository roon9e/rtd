//! RSA exchange padding, key parsing and fingerprinting.
//!
//! During the authorization-key exchange the client protects its inner data
//! with the scheme documented at
//! <https://core.telegram.org/mtproto/auth_key#presenting-proof-of-work-server-authentication>:
//! the data is padded to 192 bytes, reversed, combined with a `SHA256` over a
//! random temporary key, encrypted with AES-256-IGE under that temporary key and
//! finally RSA-encrypted with the server's public key.
//!
//! [`rsa_pad`] implements the client transform and [`decode_rsa_pad`] the
//! server-side inverse.

use core::cmp::Ordering;

use ::rsa::BigUint;
use ::rsa::RsaPrivateKey;
use ::rsa::RsaPublicKey;
use ::rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey};
use ::rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use ::rsa::traits::{PrivateKeyParts, PublicKeyParts};
use rtd_bin::Writer;
use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::error::{Error, Result};
use crate::ige;
use crate::rand::RandomSource;

/// RSA key size in bits used by MTProto.
pub const RSA_KEY_BITS: usize = 2048;
/// Maximum length of the plaintext passed to [`rsa_pad`].
pub const RSA_PAD_DATA_LIMIT: usize = 144;
/// Length of the padded plaintext (`data ++ random`).
pub const DATA_WITH_PADDING_LENGTH: usize = 192;
/// Length of the reversed padded plaintext plus its `SHA256` digest.
pub const DATA_WITH_HASH_LENGTH: usize = DATA_WITH_PADDING_LENGTH + 32;
/// Size of the random temporary AES key.
pub const TEMP_KEY_SIZE: usize = 32;
/// Length of the final RSA-encrypted block.
pub const RSA_LEN: usize = 256;

/// RSA-encrypts `data` using the MTProto exchange padding scheme.
///
/// # Errors
///
/// Returns [`Error::RsaDataTooLong`] if `data` is longer than 144 bytes.
pub fn rsa_pad(
    data: &[u8],
    key: &RsaPublicKey,
    random: &mut impl RandomSource,
) -> Result<[u8; RSA_LEN]> {
    if data.len() > RSA_PAD_DATA_LIMIT {
        return Err(Error::RsaDataTooLong { len: data.len() });
    }

    // 1) data_with_padding := data ++ random bytes, exactly 192 bytes.
    let mut data_with_padding = [0u8; DATA_WITH_PADDING_LENGTH];
    data_with_padding[..data.len()].copy_from_slice(data);
    random.fill(&mut data_with_padding[data.len()..]);

    // 2) data_pad_reversed := reverse(data_with_padding).
    let mut data_pad_reversed = data_with_padding;
    data_pad_reversed.reverse();

    loop {
        // 3) A random 32-byte temp_key is generated.
        let mut temp_key = [0u8; TEMP_KEY_SIZE];
        random.fill(&mut temp_key);

        // 4) data_with_hash := data_pad_reversed ++ SHA256(temp_key ++ data_with_padding).
        let mut data_with_hash = [0u8; DATA_WITH_HASH_LENGTH];
        data_with_hash[..DATA_WITH_PADDING_LENGTH].copy_from_slice(&data_pad_reversed);
        let mut hasher = Sha256::new();
        hasher.update(temp_key);
        hasher.update(data_with_padding);
        data_with_hash[DATA_WITH_PADDING_LENGTH..].copy_from_slice(&hasher.finalize());

        // 5) aes_encrypted := AES256_IGE(data_with_hash, temp_key, zero IV).
        let mut aes_encrypted = data_with_hash;
        ige::encrypt(&temp_key, &[0u8; 32], &mut aes_encrypted);

        // 6) temp_key_xor := temp_key XOR SHA256(aes_encrypted).
        let mut hasher = Sha256::new();
        hasher.update(aes_encrypted);
        let aes_encrypted_hash = hasher.finalize();
        let mut temp_key_xor = [0u8; TEMP_KEY_SIZE];
        for i in 0..TEMP_KEY_SIZE {
            temp_key_xor[i] = temp_key[i] ^ aes_encrypted_hash[i];
        }

        // 7) key_aes_encrypted := temp_key_xor ++ aes_encrypted, exactly 256 bytes.
        let mut key_aes_encrypted = [0u8; RSA_LEN];
        key_aes_encrypted[..TEMP_KEY_SIZE].copy_from_slice(&temp_key_xor);
        key_aes_encrypted[TEMP_KEY_SIZE..].copy_from_slice(&aes_encrypted);

        // 8) Retry while the big-endian value is >= the RSA modulus.
        let value = BigUint::from_bytes_be(&key_aes_encrypted);
        if value.cmp(key.n()) != Ordering::Less {
            continue;
        }

        // 9) encrypted_data := RSA(key_aes_encrypted).
        let encrypted = value.modpow(key.e(), key.n());
        let mut out = [0u8; RSA_LEN];
        fill_bytes_be(&encrypted, &mut out);
        return Ok(out);
    }
}

/// Recovers the padded plaintext produced by [`rsa_pad`] using the private key.
///
/// The returned buffer is 192 bytes: the original (up to 144) data bytes
/// followed by the random padding. Only the caller knows the original length.
///
/// # Errors
///
/// Returns [`Error::InvalidEncryptedData`] if the RSA block cannot be unwrapped
/// within 256 bytes, or [`Error::HashMismatch`] if the embedded digest does not
/// match.
pub fn decode_rsa_pad(data: &[u8], key: &RsaPrivateKey) -> Result<Vec<u8>> {
    let cipher = BigUint::from_bytes_be(data);
    let message = cipher.modpow(key.d(), key.n());
    let message_bytes = message.to_bytes_be();
    if message_bytes.len() > RSA_LEN {
        return Err(Error::InvalidEncryptedData);
    }
    let mut encrypted_data = [0u8; RSA_LEN];
    encrypted_data[RSA_LEN - message_bytes.len()..].copy_from_slice(&message_bytes);

    let temp_key_xor = &encrypted_data[..TEMP_KEY_SIZE];
    let aes_encrypted = &encrypted_data[TEMP_KEY_SIZE..];

    let mut hasher = Sha256::new();
    hasher.update(aes_encrypted);
    let aes_encrypted_hash = hasher.finalize();
    let mut temp_key = [0u8; TEMP_KEY_SIZE];
    for i in 0..TEMP_KEY_SIZE {
        temp_key[i] = temp_key_xor[i] ^ aes_encrypted_hash[i];
    }

    let mut data_with_hash = aes_encrypted.to_vec();
    ige::decrypt(&temp_key, &[0u8; 32], &mut data_with_hash);

    let mut data_with_padding = data_with_hash[..DATA_WITH_PADDING_LENGTH].to_vec();
    data_with_padding.reverse();
    let hash = &data_with_hash[DATA_WITH_PADDING_LENGTH..];

    let mut hasher = Sha256::new();
    hasher.update(temp_key);
    hasher.update(&data_with_padding);
    if hasher.finalize().as_slice() != hash {
        return Err(Error::HashMismatch);
    }

    Ok(data_with_padding)
}

/// Computes the MTProto RSA public key fingerprint as a little-endian `i64`.
///
/// `fingerprint = SHA1(TLBytes(N) ++ TLBytes(E))[12..20]`.
#[must_use]
pub fn rsa_fingerprint(key: &RsaPublicKey) -> i64 {
    let mut writer = Writer::new();
    writer.put_bytes(&key.n().to_bytes_be());
    writer.put_bytes(&key.e().to_bytes_be());
    let digest = Sha1::digest(writer.as_slice());
    let mut fingerprint = [0u8; 8];
    fingerprint.copy_from_slice(&digest[12..20]);
    i64::from_le_bytes(fingerprint)
}

/// Parses one PEM-encoded RSA public key in either PKCS#1 or PKIX form.
///
/// # Errors
///
/// Returns [`Error::KeyParse`] if the PEM block cannot be decoded as RSA.
pub fn parse_rsa_public_key(pem: &str) -> Result<RsaPublicKey> {
    if let Ok(key) = RsaPublicKey::from_pkcs1_pem(pem) {
        return Ok(key);
    }
    RsaPublicKey::from_public_key_pem(pem).map_err(|e| Error::KeyParse(e.to_string()))
}

/// Parses every PEM-encoded RSA public key found in `pem`.
///
/// # Errors
///
/// Returns [`Error::KeyParse`] if a block cannot be decoded or if no block is
/// present.
pub fn parse_rsa_public_keys(pem: &str) -> Result<Vec<RsaPublicKey>> {
    let blocks = pem_blocks(pem);
    if blocks.is_empty() {
        return Err(Error::KeyParse("no PEM blocks found".to_owned()));
    }
    blocks
        .iter()
        .map(|block| parse_rsa_public_key(block))
        .collect()
}

/// Parses one PEM-encoded RSA private key in either PKCS#1 or PKCS#8 form.
///
/// # Errors
///
/// Returns [`Error::KeyParse`] if the PEM block cannot be decoded as RSA.
pub fn parse_rsa_private_key(pem: &str) -> Result<RsaPrivateKey> {
    if let Ok(key) = RsaPrivateKey::from_pkcs1_pem(pem) {
        return Ok(key);
    }
    RsaPrivateKey::from_pkcs8_pem(pem).map_err(|e| Error::KeyParse(e.to_string()))
}

/// Writes `value` right-aligned into `out`, zero-filling the leading bytes.
fn fill_bytes_be(value: &BigUint, out: &mut [u8]) {
    let bytes = value.to_bytes_be();
    let start = out.len() - bytes.len();
    out[start..].copy_from_slice(&bytes);
}

/// Splits a PEM document into individual blocks (with their labels and
/// newlines) so that multiple concatenated keys can be parsed.
fn pem_blocks(data: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut in_block = false;
    for line in data.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("-----BEGIN ") {
            in_block = true;
        }
        if in_block {
            current.push_str(trimmed);
            current.push('\n');
        }
        if trimmed.starts_with("-----END ") {
            in_block = false;
            blocks.push(core::mem::take(&mut current));
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::{
        decode_rsa_pad, parse_rsa_public_key, parse_rsa_public_keys, rsa_fingerprint, rsa_pad,
    };
    use crate::error::Error;
    use crate::rand::SystemRandom;
    use crate::testutil::ZeroRandom;
    use ::rsa::RsaPrivateKey;

    const TEST_PUBLIC_KEY: &str = "\
-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEA6LszBcC1LGzyr992NzE0ieY+BSaOW622Aa9Bd4ZHLl+TuFQ4lo4g
5nKaMBwK/BIb9xUfg0Q29/2mgIR6Zr9krM7HjuIcCzFvDtr+L0GQjae9H0pRB2OO
62cECs5HKhT5DZ98K33vmWiLowc621dQuwKWSQKjWf50XYFw42h21P2KXUGyp2y/
+aEyZ+uVgLLQbRA1dEjSDZ2iGRy12Mk5gpYc397aYp438fsJoHIgJ2lgMv5h7WY9
t6N/byY9Nw9p21Og3AoXSL2q/2IJ1WRUhebgAdGVMlV1fkuOQoEzR7EdpqtQD9Cs
5+bfo3Nhmcyvk5ftB0WkJ9z6bNZ7yxrP8wIDAQAB
-----END RSA PUBLIC KEY-----
";

    #[test]
    fn known_rsa_pad_vector() {
        // Vector from the reference Go implementation with a zero random
        // source and 144 bytes of 'a'.
        let key = parse_rsa_public_key(TEST_PUBLIC_KEY).unwrap();
        let data = vec![b'a'; 144];
        let encrypted = rsa_pad(&data, &key, &mut ZeroRandom).unwrap();
        let expected = hex::decode(concat!(
            "bf68719e836806b040cd261ecaf66eb3c4ba19f3bbea3031b2e6cf29167bab647201d101b291dc",
            "5b716a42e789a38d947fe59e9bcce8f30ef46a946743ea8b6babbce7fc0afc46b802aa453e83471d82a4dfad83f971f35",
            "0b4b4fb474cd1c48fdf427e4b5fecce9ec3178ae7dac3985856fdefa21d6fdc5e0e0fd8a57bc4f51580d637d372be8d87",
            "c9aa3fde8e6f8287bcb3be846aadcdd59465375479e248f62ed438f9804fbe36d41ca906243a5f740f3937949aa149ba8",
            "a8b8e68b3f3e1e3cd3f946387520e21eee55845e1f015a919a22f6a72bfaecd2cae946c91983b41f9ffabe97963bbde8f",
            "30eaf5fd3c5b8cecab8711bd269e441b6084f385726ff0",
        ))
        .unwrap();
        assert_eq!(encrypted.as_slice(), expected.as_slice());
    }

    #[test]
    fn fingerprint_of_known_key() {
        let key = parse_rsa_public_key(TEST_PUBLIC_KEY).unwrap();
        assert_eq!(rsa_fingerprint(&key), -3_414_540_481_677_951_611);
    }

    #[test]
    fn multiple_public_keys() {
        let keys = parse_rsa_public_keys(&format!("{TEST_PUBLIC_KEY}{TEST_PUBLIC_KEY}")).unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn rejects_too_long_data() {
        let key = parse_rsa_public_key(TEST_PUBLIC_KEY).unwrap();
        let data = vec![0u8; 145];
        assert_eq!(
            rsa_pad(&data, &key, &mut ZeroRandom).unwrap_err(),
            Error::RsaDataTooLong { len: 145 }
        );
    }

    /// Generates one 2048-bit key per test process; key generation is expensive.
    fn generated_private_key() -> &'static RsaPrivateKey {
        static KEY: std::sync::OnceLock<RsaPrivateKey> = std::sync::OnceLock::new();
        KEY.get_or_init(|| {
            let mut rng = rand::rngs::OsRng;
            RsaPrivateKey::new(&mut rng, super::RSA_KEY_BITS).unwrap()
        })
    }

    #[test]
    fn generated_key_round_trip() {
        let private = generated_private_key();
        let public = private.to_public_key();
        let data = b"the quick brown fox jumps over the lazy dog, repeated to satisfy alignment!";

        let encrypted = rsa_pad(data, &public, &mut SystemRandom).unwrap();
        let decrypted = decode_rsa_pad(&encrypted, private).unwrap();
        assert_eq!(&decrypted[..data.len()], data);
        assert_eq!(decrypted.len(), super::DATA_WITH_PADDING_LENGTH);
    }

    #[test]
    fn tampered_block_is_rejected() {
        let private = generated_private_key();
        let public = private.to_public_key();
        let mut encrypted = rsa_pad(b"hello", &public, &mut SystemRandom).unwrap();
        encrypted[0] ^= 0x01;
        assert_eq!(
            decode_rsa_pad(&encrypted, private).unwrap_err(),
            Error::HashMismatch
        );
    }
}
