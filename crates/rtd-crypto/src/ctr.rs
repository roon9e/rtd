//! AES-256 in counter (CTR) mode.
//!
//! MTProto's CDN file encryption uses AES-256-CTR with the block counter
//! carried in the last four bytes of the initialization vector. This module
//! implements the mode on top of the raw AES block cipher so the downloader can
//! decrypt CDN chunks without a mode crate.

use aes::Aes256;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::generic_array::typenum::U16;
use aes::cipher::{BlockEncrypt, KeyInit};

/// The AES block size in bytes.
const BLOCK: usize = 16;

/// XORs `data` in place with the AES-256-CTR keystream starting at `iv`.
///
/// The 128-bit counter is incremented big-endian after each block.
pub fn xor(key: &[u8; 32], iv: &[u8; 16], data: &mut [u8]) {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut counter = *iv;
    for chunk in data.chunks_mut(BLOCK) {
        let mut block = GenericArray::<u8, U16>::clone_from_slice(&counter);
        cipher.encrypt_block(&mut block);
        for (byte, key_byte) in chunk.iter_mut().zip(block.iter()) {
            *byte ^= key_byte;
        }
        increment(&mut counter);
    }
}

/// Increments a 128-bit big-endian counter, wrapping on overflow.
fn increment(counter: &mut [u8; BLOCK]) {
    for byte in counter.iter_mut().rev() {
        let (next, overflow) = byte.overflowing_add(1);
        *byte = next;
        if !overflow {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::xor;

    /// NIST SP 800-38A F.5.5 AES-256-CTR vector.
    #[test]
    fn known_nist_vector() {
        let key: [u8; 32] = [
            0x60, 0x3d, 0xeb, 0x10, 0x15, 0xca, 0x71, 0xbe, 0x2b, 0x73, 0xae, 0xf0, 0x85, 0x7d,
            0x77, 0x81, 0x1f, 0x35, 0x2c, 0x07, 0x3b, 0x61, 0x08, 0xd7, 0x2d, 0x98, 0x10, 0xa3,
            0x09, 0x14, 0xdf, 0xf4,
        ];
        let iv: [u8; 16] = [
            0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd,
            0xfe, 0xff,
        ];
        let plain: [u8; 16] = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a,
        ];
        let expected: [u8; 16] = [
            0x60, 0x1e, 0xc3, 0x13, 0x77, 0x57, 0x89, 0xa5, 0xb7, 0xa7, 0xf5, 0x04, 0xbb, 0xf3,
            0xd2, 0x28,
        ];

        let mut data = plain;
        xor(&key, &iv, &mut data);
        assert_eq!(data, expected);
        // CTR is its own inverse.
        xor(&key, &iv, &mut data);
        assert_eq!(data, plain);
    }
}
