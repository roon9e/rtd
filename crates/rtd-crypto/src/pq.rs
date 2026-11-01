//! Factorization of the client's composite proof-of-work number.
//!
//! During the authorization-key exchange the server asks the client to factor a
//! composite `pq` no larger than `2^63` into its two prime factors. This is the
//! `splitPQ` algorithm used across MTProto implementations: a randomized
//! Pollard-style GCD search with an increasing iteration budget.

use num_bigint::BigUint;

use crate::rand::RandomSource;

/// Decomposes `pq` into two prime factors such that `p < q`.
///
/// The algorithm is randomized and draws its seeds from `random`. The result is
/// unique because `pq` is a product of exactly two primes.
///
/// # Panics
///
/// Panics if `pq` is less than 2.
#[must_use]
pub fn decompose_pq(pq: &BigUint, random: &mut impl RandomSource) -> (BigUint, BigUint) {
    let zero = BigUint::from(0u32);
    let one = BigUint::from(1u32);
    let seventeen = BigUint::from(17u32);
    assert!(pq > &one, "pq must be greater than one");

    let what = pq.clone();
    let mut g = zero.clone();
    let mut i: u32 = 0;
    while !(g > one && g < what) {
        let mut v = BigUint::from(next_u64(random) & 15) + &seventeen;
        v %= &what;

        let what_next = &what - &one;
        let mut x = BigUint::from(next_u64(random)) % &what_next;
        x += &one;

        let mut y = x.clone();
        let shift = (i + 18).min(127);
        let limit: u128 = 1u128 << shift;
        let mut j: u128 = 1;
        let mut flag = true;

        while j < limit && flag {
            let mut a = x.clone();
            let mut b = x.clone();
            let mut c = v.clone();
            while b > zero {
                if b.bit(0) {
                    c += &a;
                    if c >= what {
                        c -= &what;
                    }
                }
                a <<= 1u32;
                if a >= what {
                    a -= &what;
                }
                b >>= 1u32;
            }
            x = c;

            let z = if x < y { &what + &x - &y } else { &x - &y };
            g = gcd(z, what.clone());

            if j & (j - 1) == 0 {
                y = x.clone();
            }
            j += 1;
            if g != one {
                flag = false;
            }
        }
        i += 1;
    }

    let mut p = g;
    let mut q = &what / &p;
    if p > q {
        core::mem::swap(&mut p, &mut q);
    }
    (p, q)
}

/// Returns a uniform random `u64`.
fn next_u64(random: &mut impl RandomSource) -> u64 {
    let mut bytes = [0u8; 8];
    random.fill(&mut bytes);
    u64::from_le_bytes(bytes)
}

/// Euclidean greatest common divisor for unsigned big integers.
fn gcd(mut a: BigUint, mut b: BigUint) -> BigUint {
    let zero = BigUint::from(0u32);
    while b != zero {
        let remainder = &a % &b;
        a = b;
        b = remainder;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::decompose_pq;
    use crate::testutil::XorShift64;
    use num_bigint::BigUint;

    #[test]
    fn factorizes_known_composites() {
        let cases: [(u64, u64, u64); 4] = [
            (1_724_114_033_281_923_457, 1_229_739_323, 1_402_015_859),
            (378_221, 613, 617),
            (15, 3, 5),
            (0x17ED_4894_1A08_F981, 0x494C_553B, 0x5391_1073),
        ];

        for (pq, expected_p, expected_q) in cases {
            let mut random = XorShift64::new(239);
            let (p, q) = decompose_pq(&BigUint::from(pq), &mut random);
            assert_eq!(p, BigUint::from(expected_p), "p for pq={pq}");
            assert_eq!(q, BigUint::from(expected_q), "q for pq={pq}");
            assert_eq!(&p * &q, BigUint::from(pq));
        }
    }

    #[test]
    fn factors_many_random_composites() {
        // Products of two small odd primes, all below 2^63.
        let primes = [101u64, 103, 107, 109, 113, 127, 131, 137];
        for (i, a) in primes.iter().enumerate() {
            for b in primes.iter().skip(i + 1) {
                let pq = a * b;
                let mut random = XorShift64::new(*a * 1000 + *b);
                let (p, q) = decompose_pq(&BigUint::from(pq), &mut random);
                assert_eq!(&p * &q, BigUint::from(pq));
                assert!(p > BigUint::from(1u32));
                assert!(q > BigUint::from(1u32));
            }
        }
    }

    #[test]
    #[should_panic(expected = "pq must be greater than one")]
    fn rejects_trivial_input() {
        let mut random = XorShift64::new(1);
        let _ = decompose_pq(&BigUint::from(1u32), &mut random);
    }
}
