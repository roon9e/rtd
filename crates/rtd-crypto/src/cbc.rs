//! AES-256 in cipher block chaining (CBC) mode.
//!
//! MTProto itself does not use CBC, but the Telegram DNS resolver does: it
//! unwraps the encrypted simple config with AES-256-CBC under a key and IV
//! derived from the RSA-decrypted block. This module implements the mode on top
//! of the raw AES block cipher so the resolver does not need a mode crate.

use aes::Aes256;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::generic_array::typenum::U16;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};

/// The AES block size in bytes.
const BLOCK: usize = 16;

/// Encrypts `data` in place with AES-256-CBC.
///
/// # Panics
///
/// Panics if `data.len()` is not a multiple of 16.
pub fn encrypt(key: &[u8; 32], iv: &[u8; 16], data: &mut [u8]) {
    assert!(
        data.len().is_multiple_of(BLOCK),
        "CBC data length must be a multiple of 16"
    );
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut prev = *iv;
    let (chunks, _) = data.as_chunks_mut::<BLOCK>();
    for chunk in chunks {
        let plain = *chunk;
        let mut block = GenericArray::<u8, U16>::clone_from_slice(chunk);
        for (byte, p) in block.iter_mut().zip(prev.iter()) {
            *byte ^= p;
        }
        cipher.encrypt_block(&mut block);
        let mut ciphertext = [0u8; BLOCK];
        ciphertext.copy_from_slice(&block);
        *chunk = ciphertext;
        prev = plain;
    }
}

/// Decrypts `data` in place with AES-256-CBC.
///
/// # Panics
///
/// Panics if `data.len()` is not a multiple of 16.
pub fn decrypt(key: &[u8; 32], iv: &[u8; 16], data: &mut [u8]) {
    assert!(
        data.len().is_multiple_of(BLOCK),
        "CBC data length must be a multiple of 16"
    );
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut prev = *iv;
    let (chunks, _) = data.as_chunks_mut::<BLOCK>();
    for chunk in chunks {
        let ciphertext = *chunk;
        let mut block = GenericArray::<u8, U16>::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        for (byte, p) in block.iter_mut().zip(prev.iter()) {
            *byte ^= p;
        }
        let mut plain = [0u8; BLOCK];
        plain.copy_from_slice(&block);
        *chunk = plain;
        prev = ciphertext;
    }
}

#[cfg(test)]
mod tests {
    use super::{decrypt, encrypt};

    /// NIST SP 800-38A AES-256-CBC vector (F.2.5/F.2.6).
    #[test]
    fn known_nist_vector() {
        let key: [u8; 32] = [
            0x60, 0x3d, 0xeb, 0x10, 0x15, 0xca, 0x71, 0xbe, 0x2b, 0x73, 0xae, 0xf0, 0x85, 0x7d,
            0x77, 0x81, 0x1f, 0x35, 0x2c, 0x07, 0x3b, 0x61, 0x08, 0xd7, 0x2d, 0x98, 0x10, 0xa3,
            0x09, 0x14, 0xdf, 0xf4,
        ];
        let iv: [u8; 16] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let plain: [u8; 16] = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a,
        ];
        let expected: [u8; 16] = [
            0xf5, 0x8c, 0x4c, 0x04, 0xd6, 0xe5, 0xf1, 0xba, 0x77, 0x9e, 0xab, 0xfb, 0x5f, 0x7b,
            0xfb, 0xd6,
        ];

        let mut data = plain;
        encrypt(&key, &iv, &mut data);
        assert_eq!(data, expected);
        decrypt(&key, &iv, &mut data);
        assert_eq!(data, plain);
    }
}
