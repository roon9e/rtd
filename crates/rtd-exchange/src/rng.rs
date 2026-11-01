//! Server-side parameter generation for the key exchange.

use num_bigint::BigUint;
use rtd_crypto::{DH_PRIME_BITS, RandomSource, check_gp, in_range};

use crate::error::Result;

/// Supplies the server's Diffie-Hellman parameters.
///
/// A production server selects a fresh `pq` and prime; the exchange flow only
/// requires that the values satisfy the MTProto constraints.
pub trait ServerRng {
    /// Returns the composite `pq` the client must factor.
    ///
    /// # Errors
    ///
    /// Returns an error when generation fails.
    fn pq(&mut self, random: &mut impl RandomSource) -> Result<BigUint>;

    /// Returns the 2048-bit Diffie-Hellman prime.
    ///
    /// # Errors
    ///
    /// Returns an error when generation fails.
    fn dh_prime(&mut self) -> Result<BigUint>;

    /// Returns the private exponent `a` and public value `g_a = g^a mod p`.
    ///
    /// # Errors
    ///
    /// Returns an error when generation fails.
    fn ga(
        &mut self,
        g: u32,
        p: &BigUint,
        random: &mut impl RandomSource,
    ) -> Result<(BigUint, BigUint)>;
}

/// A [`ServerRng`] with a fixed `pq` and prime, for tests and local servers.
#[derive(Debug, Default, Clone, Copy)]
pub struct TestServerRng;

impl ServerRng for TestServerRng {
    fn pq(&mut self, _random: &mut impl RandomSource) -> Result<BigUint> {
        Ok(BigUint::from(0x17ED_4894_1A08_F981u64))
    }

    fn dh_prime(&mut self) -> Result<BigUint> {
        Ok(from_hex(DH_PRIME_HEX))
    }

    fn ga(
        &mut self,
        g: u32,
        p: &BigUint,
        random: &mut impl RandomSource,
    ) -> Result<(BigUint, BigUint)> {
        check_gp(g, p)?;
        let g_big = BigUint::from(g);
        let one = BigUint::from(1u32);
        let p_minus_one = p - &one;
        let safety_min = BigUint::from(2u32).pow((DH_PRIME_BITS - 64) as u32);
        let safety_max = p - &safety_min;
        loop {
            let a = crate::util::random_biguint(random);
            let ga = g_big.modpow(&a, p);
            if in_range(&ga, &one, &p_minus_one) && in_range(&ga, &safety_min, &safety_max) {
                return Ok((a, ga));
            }
        }
    }
}

/// The 2048-bit Diffie-Hellman prime used by Telegram test servers.
const DH_PRIME_HEX: &str = "\
C71CAEB9C6B1C9048E6C522F70F13F73980D40238E3E21C14934D037563D930F\
48198A0AA7C14058229493D22530F4DBFA336F6E0AC925139543AED44CCE7C37\
20FD51F69458705AC68CD4FE6B6B13ABDC9746512969328454F18FAF8C595F64\
2477FE96BB2A941D5BCD1D4AC8CC49880708FA9B378E3C4F3A9060BEE67CF9A4\
A4A695811051907E162753B56B0F6B410DBA74D8A84B2A14B3144E0EF1284754\
FD17ED950D5965B4B9DD46582DB1178D169C6BC465B0D6FF9CA3928FEF5B9AE4\
E418FC15E83EBEA0F87FA9FF5EED70050DED2849F47BF959D956850CE929851F\
0D8115F635B105EE2E4E15D04B2454BF6F4FADF034B10403119CD8E3B92FCC5B";

/// Decodes an even-length hexadecimal string into a big-endian integer.
///
/// # Panics
///
/// Panics if the constant is malformed; it is compiled in, not user input.
fn from_hex(hex: &str) -> BigUint {
    assert!(
        hex.len().is_multiple_of(2),
        "hex constant must have even length"
    );
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let raw = hex.as_bytes();
    for pair in raw.chunks(2) {
        let high = (pair[0] as char).to_digit(16).expect("hex digit") as u8;
        let low = (pair[1] as char).to_digit(16).expect("hex digit") as u8;
        bytes.push((high << 4) | low);
    }
    BigUint::from_bytes_be(&bytes)
}
