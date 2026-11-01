//! Temporary exchange keys, nonce hash, server salt and exchange answers.
//!
//! During the authorization-key exchange, `new_nonce` (`int256`, 32 bytes) and
//! `server_nonce` (`int128`, 16 bytes) are fixed-width byte strings. They are
//! hashed at their full length: representing them as leading-zero-stripped big
//! integers would shorten the hash input whenever a nonce starts with `0x00` and
//! break interoperability. This module therefore works on fixed-size arrays
//! throughout.
//!
//! See <https://core.telegram.org/mtproto/auth_key#presenting-proof-of-work-server-authentication>.

use crate::error::{Error, Result};
use crate::ige;
use crate::key::Key;
use crate::rand::RandomSource;
use rtd_bin::Int128;
use sha1::{Digest, Sha1};

/// Length of `new_nonce` in bytes (`int256`).
pub const NEW_NONCE_LEN: usize = 32;
/// Length of `server_nonce` in bytes (`int128`).
pub const SERVER_NONCE_LEN: usize = 16;

/// Returns `SHA1(a ++ b)`.
fn sha1_pair(a: &[u8], b: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(a);
    hasher.update(b);
    hasher.finalize().into()
}

/// Derives `tmp_aes_key` and `tmp_aes_iv` from the exchange nonces.
///
/// ```text
/// tmp_aes_key = SHA1(nn ++ sn) ++ SHA1(sn ++ nn)[0..12]
/// tmp_aes_iv  = SHA1(sn ++ nn)[12..20] ++ SHA1(nn ++ nn) ++ nn[0..4]
/// ```
#[must_use]
pub fn temp_aes_keys(
    new_nonce: &[u8; NEW_NONCE_LEN],
    server_nonce: &[u8; SERVER_NONCE_LEN],
) -> ([u8; 32], [u8; 32]) {
    let key_part = sha1_pair(new_nonce, server_nonce);
    let reverse = sha1_pair(server_nonce, new_nonce);

    let mut key = [0u8; 32];
    key[..20].copy_from_slice(&key_part);
    key[20..].copy_from_slice(&reverse[..12]);

    let mut iv = [0u8; 32];
    iv[..8].copy_from_slice(&reverse[12..20]);
    iv[8..28].copy_from_slice(&sha1_pair(new_nonce, new_nonce));
    iv[28..].copy_from_slice(&new_nonce[..4]);

    (key, iv)
}

/// Computes `nonce_hash_1 = SHA1(new_nonce ++ [1] ++ SHA1(key)[0..8])[4..20]`.
#[must_use]
pub fn nonce_hash1(new_nonce: &[u8; NEW_NONCE_LEN], key: &Key) -> Int128 {
    let key_hash = Sha1::digest(key.as_bytes());
    let mut hasher = Sha1::new();
    hasher.update(new_nonce);
    hasher.update([1u8]);
    hasher.update(&key_hash[..8]);
    let digest = hasher.finalize();
    let mut hash = [0u8; 16];
    hash.copy_from_slice(&digest[4..20]);
    Int128::from_array(hash)
}

/// Computes the initial `server_salt` as the little-endian XOR of the first 8
/// bytes of both nonces.
#[must_use]
pub fn server_salt(new_nonce: &[u8; NEW_NONCE_LEN], server_nonce: &[u8; SERVER_NONCE_LEN]) -> i64 {
    let mut bytes = [0u8; 8];
    for i in 0..8 {
        bytes[i] = new_nonce[i] ^ server_nonce[i];
    }
    i64::from_le_bytes(bytes)
}

/// Prepends `SHA1(data)` and random padding so the result length is a multiple
/// of 16.
///
/// Use [`guess_data_with_hash`] to recover `data`.
#[must_use]
pub fn data_with_hash(data: &[u8], random: &mut impl RandomSource) -> Vec<u8> {
    let total = padded_len_16(data.len() + 20);
    let mut out = vec![0u8; total];
    let digest = Sha1::digest(data);
    out[..20].copy_from_slice(&digest);
    out[20..20 + data.len()].copy_from_slice(data);
    random.fill(&mut out[20 + data.len()..]);
    out
}

/// Recovers the original data from a [`data_with_hash`] buffer by trying every
/// admissible padding length.
#[must_use]
pub fn guess_data_with_hash(data_with_hash: &[u8]) -> Option<&[u8]> {
    if data_with_hash.len() <= 20 {
        return None;
    }
    let expected = &data_with_hash[..20];
    for padding in 0..16 {
        if data_with_hash.len() < 20 + padding {
            return None;
        }
        let data = &data_with_hash[20..data_with_hash.len() - padding];
        let digest = Sha1::digest(data);
        if digest[..] == *expected {
            return Some(data);
        }
    }
    None
}

/// Encrypts an exchange answer with the temporary AES-IGE key.
#[must_use]
pub fn encrypt_exchange_answer(
    random: &mut impl RandomSource,
    answer: &[u8],
    key: &[u8; 32],
    iv: &[u8; 32],
) -> Vec<u8> {
    let mut data = data_with_hash(answer, random);
    ige::encrypt(key, iv, &mut data);
    data
}

/// Decrypts and unwraps an exchange answer with the temporary AES-IGE key.
///
/// # Errors
///
/// Returns [`Error::InvalidBlockLength`] if `data` is not a multiple of 16
/// bytes, or [`Error::HashMismatch`] if no valid `SHA1` prefix can be found.
pub fn decrypt_exchange_answer(data: &[u8], key: &[u8; 32], iv: &[u8; 32]) -> Result<Vec<u8>> {
    if !data.len().is_multiple_of(16) {
        return Err(Error::InvalidBlockLength { len: data.len() });
    }
    let mut buffer = data.to_vec();
    ige::decrypt(key, iv, &mut buffer);
    guess_data_with_hash(&buffer)
        .map(<[u8]>::to_vec)
        .ok_or(Error::HashMismatch)
}

/// Rounds `len` up to a multiple of 16.
fn padded_len_16(len: usize) -> usize {
    let rounded = 16 * (len / 16);
    if rounded < len { rounded + 16 } else { rounded }
}

#[cfg(test)]
mod tests {
    use super::{
        decrypt_exchange_answer, encrypt_exchange_answer, nonce_hash1, server_salt, temp_aes_keys,
    };
    use crate::key::Key;
    use crate::testutil::RepeatRandom;
    use crate::testutil::XorShift64;

    fn new_nonce_sequential() -> [u8; 32] {
        let mut nonce = [0u8; 32];
        for (i, byte) in nonce.iter_mut().enumerate() {
            *byte = i as u8;
        }
        nonce
    }

    #[test]
    fn known_temp_aes_keys() {
        // Vector from https://core.telegram.org/mtproto/samples-auth_key.
        let new_nonce: [u8; 32] =
            hex::decode("311C85DB234AA2640AFC4A76A735CF5B1F0FD68BD17FA181E1229AD867CC024D")
                .unwrap()
                .try_into()
                .unwrap();
        let server_nonce: [u8; 16] = hex::decode("A5CF4D33F4A11EA877BA4AA573907330")
            .unwrap()
            .try_into()
            .unwrap();

        let (key, iv) = temp_aes_keys(&new_nonce, &server_nonce);
        assert_eq!(
            key,
            hex_array("F011280887C7BB01DF0FC4E17830E0B91FBB8BE4B2267CB985AE25F33B527253")
        );
        assert_eq!(
            iv,
            hex_array("3212D579EE35452ED23E0D0C92841AA7D31B2E9BDEF2151E80D15860311C85DB")
        );
    }

    #[test]
    fn leading_zero_nonce_is_preserved() {
        let new_nonce = new_nonce_sequential();
        let mut server_nonce = [0u8; 16];
        for (i, byte) in server_nonce.iter_mut().enumerate() {
            *byte = 0x20 + i as u8;
        }
        server_nonce[0] = 0x00;

        let (key, iv) = temp_aes_keys(&new_nonce, &server_nonce);
        assert_eq!(
            key,
            hex_array("0a2d0f607499a866848e41798a34315e4922c39fe55c6caf5bc322c4b8c08c6a")
        );
        assert_eq!(
            iv,
            hex_array("9cb702d4fd6e5d25e864070e166e6218f6783e8511471a5ab7802cf200010203")
        );
        // tmp_aes_iv ends with the first four (untrimmed) nonce bytes.
        assert_eq!(&iv[28..], &new_nonce[..4]);
    }

    #[test]
    fn known_nonce_hash1_and_salt() {
        let new_nonce = new_nonce_sequential();
        let mut key_bytes = [0u8; 256];
        for (i, byte) in key_bytes.iter_mut().enumerate() {
            *byte = i as u8;
        }
        let key = Key::from_array(key_bytes);
        assert_eq!(
            nonce_hash1(&new_nonce, &key).as_array(),
            &hex_array::<16>("c2ced2b33e593a55d27f4a5dabee7c67")
        );

        let server_nonce: [u8; 16] = [
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 1, 2, 3, 4, 5, 6, 7, 8,
        ];
        assert_eq!(
            server_salt(&new_nonce, &server_nonce),
            2_672_880_790_535_060_138
        );
    }

    #[test]
    fn exchange_answer_round_trip() {
        let key = [0x11u8; 32];
        let iv = [0x22u8; 32];
        let answer = b"exchange answer payload";
        let encrypted = encrypt_exchange_answer(&mut XorShift64::new(7), answer, &key, &iv);
        assert_eq!(encrypted.len() % 16, 0);
        let decrypted = decrypt_exchange_answer(&encrypted, &key, &iv).unwrap();
        assert_eq!(decrypted, answer);
    }

    #[test]
    fn known_exchange_answer_decrypt() {
        // Sample authorization-key exchange from the Telegram documentation.
        let encrypted = hex::decode(concat!(
            "28A92FE20173B347A8BB324B5FAB2667C9A8BBCE6468D5B509A4CB",
            "DDC186240AC912CF7006AF8926DE606A2E74C0493CAA57741E6C82451F54D3E068F5CCC49B4444124B966",
            "6FFB405AAB564A3D01E67F6E912867C8D20D9882707DC330B17B4E0DD57CB53BFAAFA9EF5BE76AE6C1B9B6",
            "C51E2D6502A47C883095C46C81E3BE25F62427B585488BB3BF239213BF48EB8FE34C9A026CC8413934043",
            "974DB03556633038392CECB51F94824E140B98637730A4BE79A8F9DAFA39BAE81E1095849EA4C83467C9",
            "2A3A17D997817C8A7AC61C3FF414DA37B7D66E949C0AEC858F048224210FCC61F11C3A910B431CCBD104",
            "CCCC8DC6D29D4A5D133BE639A4C32BBFF153E63ACA3AC52F2E4709B8AE01844B142C1EE89D075D64F69A",
            "399FEB04E656FE3675A6F8F412078F3D0B58DA15311C1A9F8E53B3CD6BB5572C294904B726D0BE337E2E2",
            "1977DA26DD6E33270251C2CA29DFCC70227F0755F84CFDA9AC4B8DD5F84F1D1EB36BA45CDDC70444D8C21",
            "3E4BD8F63B8AB95A2D0B4180DC91283DC063ACFB92D6A4E407CDE7C8C69689F77A007441D4A6A8384B666",
            "502D9B77FC68B5B43CC607E60A146223E110FCB43BC3C942EF981930CDC4A1D310C0B64D5E55D308D86325",
            "1AB90502C3E46CC599E886A927CDA963B9EB16CE62603B68529EE98F9F5206419E03FB458EC4BD9454AA8F6",
            "BA777573CC54B328895B1DF25EAD9FB4CD5198EE022B2B81F388D281D5E5BC580107CA01A50665C32B55271",
            "5F335FD76264FAD00DDD5AE45B94832AC79CE7C511D194BC42B70EFA850BB15C2012C5215CABFE97CE66B8D8",
            "734D0EE759A638AF013",
        ))
        .unwrap();
        let key = hex_array("F011280887C7BB01DF0FC4E17830E0B91FBB8BE4B2267CB985AE25F33B527253");
        let iv = hex_array("3212D579EE35452ED23E0D0C92841AA7D31B2E9BDEF2151E80D15860311C85DB");

        let answer = decrypt_exchange_answer(&encrypted, &key, &iv).unwrap();
        let expected = hex::decode(concat!(
            "BA0D89B53E0549828CCA27E966B301A48FECE2FCA5CF4D33F4A1",
            "1EA877BA4AA57390733002000000FE000100C71CAEB9C6B1C9048E6C522F70F13F73980D40238E3E2",
            "1C14934D037563D930F48198A0AA7C14058229493D22530F4DBFA336F6E0AC925139543AED44CCE7C3",
            "720FD51F69458705AC68CD4FE6B6B13ABDC9746512969328454F18FAF8C595F642477FE96BB2A941D5",
            "BCD1D4AC8CC49880708FA9B378E3C4F3A9060BEE67CF9A4A4A695811051907E162753B56B0F6B410DBA",
            "74D8A84B2A14B3144E0EF1284754FD17ED950D5965B4B9DD46582DB1178D169C6BC465B0D6FF9CA392",
            "8FEF5B9AE4E418FC15E83EBEA0F87FA9FF5EED70050DED2849F47BF959D956850CE929851F0D8115F63",
            "5B105EE2E4E15D04B2454BF6F4FADF034B10403119CD8E3B92FCC5BFE000100262AABA621CC4DF587D",
            "C94CF8252258C0B9337DFB47545A49CDD5C9B8EAE7236C6CADC40B24E88590F1CC2CC762EBF1CF11DC",
            "C0B393CAAD6CEE4EE5848001C73ACBB1D127E4CB93072AA3D1C8151B6FB6AA6124B7CD782EAF981BDCF",
            "CE9D7A00E423BD9D194E8AF78EF6501F415522E44522281C79D906DDB79C72E9C63D83FB2A940FF77",
            "9DFB5F2FD786FB4AD71C9F08CF48758E534E9815F634F1E3A80A5E1C2AF210C5AB762755AD4B2126DF",
            "A61A77FA9DA967D65DFD0AFB5CDF26C4D4E1A88B180F4E0D0B45BA1484F95CB2712B50BF3F5968D9D5",
            "5C99C0FB9FB67BFF56D7D4481B634514FBA3488C4CDA2FC0659990E8E868B28632875A9AA703BCDCE8FCB7AE551",
        ))
        .unwrap();
        assert_eq!(answer, expected);
    }

    #[test]
    fn invalid_length_is_rejected() {
        assert!(decrypt_exchange_answer(&[0u8; 17], &[0u8; 32], &[0u8; 32]).is_err());
    }

    #[test]
    fn repeat_random_is_deterministic() {
        let mut random = RepeatRandom { byte: 0x5A };
        let out = super::data_with_hash(b"data", &mut random);
        assert_eq!(out.len() % 16, 0);
        assert_eq!(super::guess_data_with_hash(&out).unwrap(), b"data");
    }

    fn hex_array<const N: usize>(hex: &str) -> [u8; N] {
        hex::decode(hex)
            .expect("valid hex")
            .try_into()
            .expect("correct length")
    }
}
