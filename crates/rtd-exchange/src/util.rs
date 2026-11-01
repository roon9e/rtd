//! Shared wire and random helpers for the exchange flows.

use num_bigint::BigUint;
use rtd_bin::{Int128, Int256};
use rtd_crypto::{Key, RandomSource};
use rtd_mt::{DecodeLimits, Decoder};

use crate::error::{Error, Result};

/// Decodes a TL value from `data` with default limits.
pub(crate) fn decode_with<'a, T>(
    data: &'a [u8],
    decode: impl FnOnce(&mut Decoder<'a>) -> rtd_bin::Result<T>,
) -> Result<T> {
    let mut decoder = Decoder::new(data, DecodeLimits::default());
    decode(&mut decoder).map_err(Error::Bin)
}

/// Reads the constructor id from the start of a message.
pub(crate) fn peek_id(data: &[u8]) -> Result<u32> {
    let bytes: [u8; 4] = data
        .get(..4)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| Error::Decode("message shorter than a constructor id".to_owned()))?;
    Ok(u32::from_le_bytes(bytes))
}

/// Returns a random `int128`.
pub(crate) fn random_int128(random: &mut impl RandomSource) -> Int128 {
    let mut bytes = [0u8; Int128::LEN];
    random.fill(&mut bytes);
    Int128::from_array(bytes)
}

/// Returns a random `int256`.
pub(crate) fn random_int256(random: &mut impl RandomSource) -> Int256 {
    let mut bytes = [0u8; Int256::LEN];
    random.fill(&mut bytes);
    Int256::from_array(bytes)
}

/// Returns a random `long`.
pub(crate) fn random_i64(random: &mut impl RandomSource) -> i64 {
    let mut bytes = [0u8; 8];
    random.fill(&mut bytes);
    i64::from_le_bytes(bytes)
}

/// Returns a random 2048-bit integer.
pub(crate) fn random_biguint(random: &mut impl RandomSource) -> BigUint {
    let mut bytes = [0u8; 256];
    random.fill(&mut bytes);
    BigUint::from_bytes_be(&bytes)
}

/// Fills a 2048-bit authorization key from a big-endian integer.
///
/// # Errors
///
/// Returns [`Error::KeyTooLarge`] when the value does not fit the key size.
pub(crate) fn biguint_to_key(value: &BigUint) -> Result<Key> {
    let bytes = value.to_bytes_be();
    if bytes.len() > Key::LEN {
        return Err(Error::KeyTooLarge);
    }
    let mut key = [0u8; Key::LEN];
    key[Key::LEN - bytes.len()..].copy_from_slice(&bytes);
    Ok(Key::from_array(key))
}
