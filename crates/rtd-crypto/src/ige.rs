//! AES-256 in infinite-garble-extension (IGE) mode.
//!
//! IGE is the block cipher mode used by MTProto. It is defined by
//!
//! ```text
//! c_i = E_k(p_i XOR c_{i-1}) XOR p_{i-1}
//! ```
//!
//! where `c_{-1}` is the first half of the initialization vector and `p_{-1}`
//! is its second half. The IV is therefore two AES blocks (32 bytes) long, not
//! one: [`encrypt`] and [`decrypt`] take `&[u8; 32]`.
//!
//! Both functions operate in place and require `data.len()` to be a multiple of
//! the 16-byte AES block size. This module deliberately reimplements the mode on
//! top of the raw AES block cipher from the [`aes`] crate rather than relying on
//! a mode crate, matching the reference implementation exactly.

use aes::Aes256;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::generic_array::typenum::U16;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};

/// The AES block size in bytes.
const BLOCK: usize = 16;

/// Encrypts `data` in place with AES-256-IGE.
///
/// # Panics
///
/// Panics if `data.len()` is not a multiple of 16.
pub fn encrypt(key: &[u8; 32], iv: &[u8; 32], data: &mut [u8]) {
    let cipher = Aes256::new_from_slice(key).expect("AES-256 accepts a 32-byte key");
    encrypt_blocks(&cipher, iv, data);
}

/// Decrypts `data` in place with AES-256-IGE.
///
/// # Panics
///
/// Panics if `data.len()` is not a multiple of 16.
pub fn decrypt(key: &[u8; 32], iv: &[u8; 32], data: &mut [u8]) {
    let cipher = Aes256::new_from_slice(key).expect("AES-256 accepts a 32-byte key");
    decrypt_blocks(&cipher, iv, data);
}

/// XORs `src` into `dst`, element by element.
fn xor_into(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst.iter_mut().zip(src.iter()) {
        *d ^= s;
    }
}

/// Encrypts fixed 16-byte blocks in place.
fn encrypt_blocks<C>(cipher: &C, iv: &[u8; 32], data: &mut [u8])
where
    C: BlockEncrypt<BlockSize = U16>,
{
    assert!(
        data.len().is_multiple_of(BLOCK),
        "IGE data length must be a multiple of 16"
    );
    let mut prev_cipher = [0u8; BLOCK];
    prev_cipher.copy_from_slice(&iv[..BLOCK]);
    let mut prev_plain = [0u8; BLOCK];
    prev_plain.copy_from_slice(&iv[BLOCK..]);
    let (chunks, _) = data.as_chunks_mut::<BLOCK>();
    for chunk in chunks {
        let plain = *chunk;
        let mut block = GenericArray::<u8, U16>::clone_from_slice(chunk);
        xor_into(&mut block, &prev_cipher);
        cipher.encrypt_block(&mut block);
        xor_into(&mut block, &prev_plain);
        let mut ciphertext = [0u8; BLOCK];
        ciphertext.copy_from_slice(&block);
        *chunk = ciphertext;
        prev_cipher = ciphertext;
        prev_plain = plain;
    }
}

/// Decrypts fixed 16-byte blocks in place.
fn decrypt_blocks<C>(cipher: &C, iv: &[u8; 32], data: &mut [u8])
where
    C: BlockDecrypt<BlockSize = U16>,
{
    assert!(
        data.len().is_multiple_of(BLOCK),
        "IGE data length must be a multiple of 16"
    );
    let mut prev_cipher = [0u8; BLOCK];
    prev_cipher.copy_from_slice(&iv[..BLOCK]);
    let mut prev_plain = [0u8; BLOCK];
    prev_plain.copy_from_slice(&iv[BLOCK..]);
    let (chunks, _) = data.as_chunks_mut::<BLOCK>();
    for chunk in chunks {
        let ciphertext = *chunk;
        let mut block = GenericArray::<u8, U16>::clone_from_slice(chunk);
        xor_into(&mut block, &prev_plain);
        cipher.decrypt_block(&mut block);
        xor_into(&mut block, &prev_cipher);
        let mut plain = [0u8; BLOCK];
        plain.copy_from_slice(&block);
        *chunk = plain;
        prev_cipher = ciphertext;
        prev_plain = plain;
    }
}

#[cfg(test)]
mod tests {
    use super::{decrypt, decrypt_blocks, encrypt, encrypt_blocks};
    use aes::Aes128;
    use aes::cipher::KeyInit;

    fn array<const N: usize>(hex: &str) -> [u8; N] {
        hex::decode(hex)
            .expect("valid hex")
            .try_into()
            .expect("correct length")
    }

    /// Official OpenSSL IGE test vector 1 (AES-128).
    #[test]
    fn known_openssl_ige_vector_1() {
        let key: [u8; 16] = array("000102030405060708090a0b0c0d0e0f");
        let iv: [u8; 32] =
            array("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        let plain = [0u8; 32];
        let expected: [u8; 32] =
            array("1a8519a6557be652e9da8e43da4ef4453cf456b4ca488aa383c79c98b34797cb");

        let cipher = Aes128::new_from_slice(&key).unwrap();
        let mut data = plain;
        encrypt_blocks(&cipher, &iv, &mut data);
        assert_eq!(data, expected);

        decrypt_blocks(&cipher, &iv, &mut data);
        assert_eq!(data, plain);
    }

    /// Official OpenSSL IGE test vector 2 (AES-128).
    #[test]
    fn known_openssl_ige_vector_2() {
        let key: [u8; 16] = array("5468697320697320616e20696d706c65");
        let iv: [u8; 32] =
            array("6d656e746174696f6e206f6620494745206d6f646520666f72204f70656e5353");
        let plain: [u8; 32] =
            array("99706487a1cde613bc6de0b6f24b1c7aa448c8b9c3403e3467a8cad89340f53b");
        let expected: [u8; 32] =
            array("4c2e204c6574277320686f70652042656e20676f74206974207269676874210a");

        let cipher = Aes128::new_from_slice(&key).unwrap();
        let mut data = plain;
        encrypt_blocks(&cipher, &iv, &mut data);
        assert_eq!(data, expected);

        decrypt_blocks(&cipher, &iv, &mut data);
        assert_eq!(data, plain);
    }

    /// AES-256 round-trip.
    #[test]
    fn aes256_round_trip() {
        let key = [0x42u8; 32];
        let iv = [0x24u8; 32];
        let plain: Vec<u8> = (0..160).map(|i| (i * 7 + 3) as u8).collect();

        let mut data = plain.clone();
        encrypt(&key, &iv, &mut data);
        assert_ne!(data, plain);

        decrypt(&key, &iv, &mut data);
        assert_eq!(data, plain);
    }
}
