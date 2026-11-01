//! Diffie-Hellman parameter validation.
//!
//! Before using server-provided Diffie-Hellman parameters the client must check
//! that `p` is a safe 2048-bit prime, that `g` generates the prime-order
//! subgroup, and that the public values lie in the expected ranges. See
//! <https://core.telegram.org/mtproto/auth_key#presenting-proof-of-work-server-authentication>.

use num_bigint::BigUint;
use num_traits::ToPrimitive;

use crate::error::{Error, Result};
use crate::rand::{RandomSource, SystemRandom};

/// The number of bits in the MTProto Diffie-Hellman prime.
pub const DH_PRIME_BITS: u64 = 2048;

/// The number of Miller-Rabin rounds used for primality testing.
const MILLER_RABIN_ROUNDS: u32 = 64;

/// Reports whether `x` lies strictly between `min` and `max`.
#[must_use]
pub fn in_range(x: &BigUint, min: &BigUint, max: &BigUint) -> bool {
    x > min && x < max
}

/// Checks that `g`, `g_a` and `g_b` meet the key-exchange conditions.
///
/// # Errors
///
/// Returns [`Error::DhParams`] when any value is out of range.
pub fn check_dh_params(
    dh_prime: &BigUint,
    g: &BigUint,
    g_a: &BigUint,
    g_b: &BigUint,
) -> Result<()> {
    let one = BigUint::from(1u32);
    let dh_prime_minus_one = dh_prime - &one;

    if !in_range(g, &one, &dh_prime_minus_one) {
        return Err(Error::DhParams {
            reason: "g must satisfy 1 < g < dh_prime - 1",
        });
    }
    if !in_range(g_a, &one, &dh_prime_minus_one) {
        return Err(Error::DhParams {
            reason: "g_a must satisfy 1 < g_a < dh_prime - 1",
        });
    }
    if !in_range(g_b, &one, &dh_prime_minus_one) {
        return Err(Error::DhParams {
            reason: "g_b must satisfy 1 < g_b < dh_prime - 1",
        });
    }

    let safety_min = BigUint::from(2u32).pow((DH_PRIME_BITS - 64) as u32);
    let safety_max = dh_prime - &safety_min;
    if !in_range(g_a, &safety_min, &safety_max) {
        return Err(Error::DhParams {
            reason: "g_a must lie in the recommended safety range",
        });
    }
    if !in_range(g_b, &safety_min, &safety_max) {
        return Err(Error::DhParams {
            reason: "g_b must lie in the recommended safety range",
        });
    }

    Ok(())
}

/// Checks that `g` is one of `2..=7` and generates the prime-order subgroup.
///
/// # Errors
///
/// Returns [`Error::DhParams`] when `g` is unsupported or fails its
/// quadratic-residue condition on `p`.
pub fn check_gp(g: u32, p: &BigUint) -> Result<()> {
    let ok = match g {
        2 => check_subgroup(p, 8, &[7]),
        3 => check_subgroup(p, 3, &[2]),
        4 => true,
        5 => check_subgroup(p, 5, &[1, 4]),
        6 => check_subgroup(p, 24, &[19, 23]),
        7 => check_subgroup(p, 7, &[3, 5, 6]),
        _ => {
            return Err(Error::DhParams {
                reason: "g must be one of 2, 3, 4, 5, 6 or 7",
            });
        }
    };
    if !ok {
        return Err(Error::DhParams {
            reason: "g must be a quadratic residue mod p",
        });
    }
    Ok(())
}

/// Checks that `p` is a safe 2048-bit prime and that `g` is admissible.
///
/// # Errors
///
/// Returns [`Error::DhParams`] when `p` is not a 2048-bit safe prime or when
/// [`check_gp`] rejects `g`.
pub fn check_dh(g: u32, p: &BigUint) -> Result<()> {
    if p.bits() != DH_PRIME_BITS {
        return Err(Error::DhParams {
            reason: "p must be 2^2047 <= p < 2^2048",
        });
    }
    check_gp(g, p)?;
    check_prime(p)
}

/// Checks that both `p` and `(p-1)/2` are probably prime.
///
/// # Errors
///
/// Returns [`Error::DhParams`] if either fails the Miller-Rabin test.
pub fn check_prime(p: &BigUint) -> Result<()> {
    let mut random = SystemRandom;
    if !is_probably_prime(p, MILLER_RABIN_ROUNDS, &mut random) {
        return Err(Error::DhParams {
            reason: "p is not a prime number",
        });
    }
    let one = BigUint::from(1u32);
    let half = (p - &one) / BigUint::from(2u32);
    if !is_probably_prime(&half, MILLER_RABIN_ROUNDS, &mut random) {
        return Err(Error::DhParams {
            reason: "(p - 1) / 2 is not a prime number",
        });
    }
    Ok(())
}

/// Tests one of the generator congruence conditions.
fn check_subgroup(p: &BigUint, divider: u64, expected: &[u64]) -> bool {
    match (p % BigUint::from(divider)).to_u64() {
        Some(remainder) => expected.contains(&remainder),
        // The remainder is always smaller than the divider, so this is
        // unreachable in practice; treat a non-u64 remainder as not matching.
        None => false,
    }
}

/// Miller-Rabin probabilistic primality test with random witnesses.
fn is_probably_prime(n: &BigUint, rounds: u32, random: &mut impl RandomSource) -> bool {
    let two = BigUint::from(2u32);
    if n < &two {
        return false;
    }
    for small in [2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let value = BigUint::from(small);
        if n == &value {
            return true;
        }
        if n % &value == BigUint::from(0u32) {
            return false;
        }
    }

    let one = BigUint::from(1u32);
    let n_minus_one = n - &one;
    let mut d = n_minus_one.clone();
    let mut r = 0u32;
    while !d.bit(0) {
        d >>= 1u32;
        r += 1;
    }

    let range = n - BigUint::from(3u32); // Witnesses are drawn from [2, n-2].
    'witness: for _ in 0..rounds {
        let a = random_below(&range, random) + &two;
        let mut x = a.modpow(&d, n);
        if x == one || x == n_minus_one {
            continue;
        }
        for _ in 1..r {
            x = x.modpow(&two, n);
            if x == n_minus_one {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

/// Returns a uniform random value in `[0, n)`.
fn random_below(n: &BigUint, random: &mut impl RandomSource) -> BigUint {
    let zero = BigUint::from(0u32);
    if n <= &BigUint::from(1u32) {
        return zero;
    }
    let bytes = n.bits().div_ceil(8) as usize + 8;
    let mut buffer = vec![0u8; bytes];
    loop {
        random.fill(&mut buffer);
        let value = BigUint::from_bytes_be(&buffer) % n;
        if value != zero {
            return value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{check_dh, check_dh_params, check_gp, check_prime, in_range};
    use num_bigint::BigUint;

    /// The 2048-bit prime from the Telegram documentation example.
    fn example_prime() -> BigUint {
        let bytes = hex::decode(concat!(
            "C71CAEB9C6B1C9048E6C522F70F13F73980D40238E3E21C14934D037563D930F",
            "48198A0AA7C14058229493D22530F4DBFA336F6E0AC925139543AED44CCE7C37",
            "20FD51F69458705AC68CD4FE6B6B13ABDC9746512969328454F18FAF8C595F64",
            "2477FE96BB2A941D5BCD1D4AC8CC49880708FA9B378E3C4F3A9060BEE67CF9A4",
            "A4A695811051907E162753B56B0F6B410DBA74D8A84B2A14B3144E0EF1284754",
            "FD17ED950D5965B4B9DD46582DB1178D169C6BC465B0D6FF9CA3928FEF5B9AE4",
            "E418FC15E83EBEA0F87FA9FF5EED70050DED2849F47BF959D956850CE929851F",
            "0D8115F635B105EE2E4E15D04B2454BF6F4FADF034B10403119CD8E3B92FCC5B",
        ))
        .unwrap();
        BigUint::from_bytes_be(&bytes)
    }

    #[test]
    fn accepts_example_prime() {
        let p = example_prime();
        assert!(check_dh(3, &p).is_ok());
        assert!(check_gp(3, &p).is_ok());
        assert!(check_prime(&p).is_ok());
    }

    #[test]
    fn rejects_weak_parameters() {
        // Too small to be a 2048-bit prime.
        assert!(check_dh(3, &BigUint::from(4u32)).is_err());
        // Unsupported generator.
        assert!(check_dh(1337, &example_prime()).is_err());
        // g = 3 requires p % 3 == 2.
        assert!(check_gp(3, &BigUint::from(4u32)).is_err());
        // 13 is prime but (13 - 1) / 2 = 6 is not.
        assert!(check_prime(&BigUint::from(13u32)).is_err());
        // 4 is not prime.
        assert!(check_prime(&BigUint::from(4u32)).is_err());
    }

    #[test]
    fn range_is_exclusive() {
        let min = BigUint::from(10u32);
        let max = BigUint::from(20u32);
        assert!(!in_range(&BigUint::from(10u32), &min, &max));
        assert!(in_range(&BigUint::from(15u32), &min, &max));
        assert!(!in_range(&BigUint::from(20u32), &min, &max));
    }

    #[test]
    fn dh_params_reject_out_of_range() {
        let p = example_prime();
        let one = BigUint::from(1u32);
        let two = BigUint::from(2u32);
        assert!(check_dh_params(&p, &two, &one, &two).is_err());
        assert!(check_dh_params(&p, &one, &two, &two).is_err());
    }
}
