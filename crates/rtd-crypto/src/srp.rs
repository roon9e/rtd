//! SRP 2FA password hashing and proof computation.
//!
//! Implements the client side of the Secure Remote Password protocol used by
//! Telegram's two-factor authentication, as described at
//! <https://core.telegram.org/api/srp>.
//!
//! The main hashing function is `H = SHA256`. The salted variant is
//! `SH(data, salt) = H(salt ++ data ++ salt)`.

use num_bigint::BigUint;
use pbkdf2::pbkdf2_hmac;
use sha2::{Digest, Sha256, Sha512};

use crate::dh::check_dh;
use crate::error::{Error, Result};
use crate::rand::{RandomSource, SystemRandom};

/// The password-derivation parameters supplied by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    /// The first derivation salt.
    pub salt1: Vec<u8>,
    /// The second derivation salt.
    pub salt2: Vec<u8>,
    /// The generator `g`.
    pub g: i32,
    /// The 2048-bit modulus `p` in big-endian form.
    pub p: Vec<u8>,
}

/// The client's SRP proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The public value `A = g^a mod p`, padded to 256 bytes.
    pub a: Vec<u8>,
    /// The proof `M1`.
    pub m1: Vec<u8>,
}

/// Computes the SRP proof for `password` given the server's public value
/// `srp_b` and the client's secret scalar `random`.
///
/// # Errors
///
/// Returns [`Error::Srp`] or [`Error::DhParams`] if the server parameters are
/// invalid, or [`Error::IntegerTooLarge`] if an intermediate value exceeds 256
/// bytes.
pub fn hash(password: &[u8], srp_b: &[u8], random: &[u8], input: &Input) -> Result<Answer> {
    let p = BigUint::from_bytes_be(&input.p);
    let g = generator(input.g)?;
    check_dh(input.g as u32, &p)?;

    let g_bytes = pad256_from_big(&g)?;

    let a = BigUint::from_bytes_be(random);
    let g_a = pad256_from_big(&g.modpow(&a, &p))?;
    let g_b = pad256(srp_b);

    let u = BigUint::from_bytes_be(&sha256(&[&g_a, &g_b]));

    let x = BigUint::from_bytes_be(&secondary(password, &input.salt1, &input.salt2));
    let v = g.modpow(&x, &p);

    let k = BigUint::from_bytes_be(&sha256(&[&input.p, &g_bytes]));

    let k_v = (&k * &v) % &p;
    let mut t = BigUint::from_bytes_be(srp_b);
    if t >= k_v {
        t -= &k_v;
    } else {
        t = &t + &p - &k_v;
    }

    let exponent = &u * &x + &a;
    let s_a = pad256_from_big(&t.modpow(&exponent, &p))?;
    let k_a = sha256(&[&s_a]);

    let h_p = sha256(&[&input.p]);
    let h_g = sha256(&[&g_bytes]);
    let mut xor_hp_hg = [0u8; 32];
    for i in 0..32 {
        xor_hp_hg[i] = h_p[i] ^ h_g[i];
    }

    let hint_salt1 = sha256(&[&input.salt1]);
    let hint_salt2 = sha256(&[&input.salt2]);
    let m1 = sha256(&[&xor_hp_hg, &hint_salt1, &hint_salt2, &g_a, &g_b, &k_a]);

    Ok(Answer {
        a: g_a.to_vec(),
        m1: m1.to_vec(),
    })
}

/// Computes a new password hash for the given salt parameters using the system
/// CSPRNG for the fresh client salt.
///
/// Returns `(new_password_hash, new_client_salt)`.
///
/// # Errors
///
/// Returns [`Error::Srp`] or [`Error::DhParams`] if the parameters are invalid.
pub fn new_hash(password: &[u8], input: &Input) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut random = SystemRandom;
    new_hash_with_random(password, input, &mut random)
}

/// Like [`new_hash`], but draws the 32-byte client salt extension from `random`.
///
/// # Errors
///
/// Returns [`Error::Srp`] or [`Error::DhParams`] if the parameters are invalid.
pub fn new_hash_with_random(
    password: &[u8],
    input: &Input,
    random: &mut impl RandomSource,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let p = BigUint::from_bytes_be(&input.p);
    let g = generator(input.g)?;
    check_dh(input.g as u32, &p)?;

    let mut new_salt = input.salt1.clone();
    let base = new_salt.len();
    new_salt.resize(base + 32, 0);
    random.fill(&mut new_salt[base..]);

    let x = BigUint::from_bytes_be(&secondary(password, &new_salt, &input.salt2));
    let v = g.modpow(&x, &p);
    let padded = pad256_from_big(&v)?;
    Ok((padded.to_vec(), new_salt))
}

/// Converts the generator to a [`BigUint`], rejecting non-positive values.
fn generator(g: i32) -> Result<BigUint> {
    u32::try_from(g).map(BigUint::from).map_err(|_| Error::Srp {
        reason: "generator must be positive",
    })
}

/// `H(data...) = SHA256(data...)`.
fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// `SH(data, salt) = H(salt ++ data ++ salt)`.
fn salt_hash(data: &[u8], salt: &[u8]) -> [u8; 32] {
    sha256(&[salt, data, salt])
}

/// `PH1(password, salt1, salt2) = SH(SH(password, salt1), salt2)`.
fn primary(password: &[u8], salt1: &[u8], salt2: &[u8]) -> [u8; 32] {
    salt_hash(&salt_hash(password, salt1), salt2)
}

/// `PH2(password, salt1, salt2) = SH(pbkdf2_sha512(PH1, salt1, 100000, 64), salt2)`.
fn secondary(password: &[u8], salt1: &[u8], salt2: &[u8]) -> [u8; 32] {
    let ph1 = primary(password, salt1, salt2);
    let mut derived = [0u8; 64];
    pbkdf2_hmac::<Sha512>(&ph1, salt1, 100_000, &mut derived);
    salt_hash(&derived, salt2)
}

/// Right-aligns a big integer into 256 bytes.
fn pad256_from_big(value: &BigUint) -> Result<[u8; 256]> {
    let bytes = value.to_bytes_be();
    if bytes.len() > 256 {
        return Err(Error::IntegerTooLarge);
    }
    let mut out = [0u8; 256];
    out[256 - bytes.len()..].copy_from_slice(&bytes);
    Ok(out)
}

/// Left-pads (or truncates from the left) a byte string to 256 bytes.
fn pad256(bytes: &[u8]) -> [u8; 256] {
    let mut out = [0u8; 256];
    if bytes.len() >= 256 {
        out.copy_from_slice(&bytes[bytes.len() - 256..]);
    } else {
        out[256 - bytes.len()..].copy_from_slice(bytes);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Answer, Input, hash, new_hash_with_random};
    use crate::testutil::ZeroRandom;

    fn hex_vec(hex: &str) -> Vec<u8> {
        hex::decode(hex).expect("valid hex")
    }

    /// Returns 256 bytes whose big-endian value is `value`.
    fn set_byte(value: u32) -> Vec<u8> {
        let mut out = vec![0u8; 256];
        out[252..].copy_from_slice(&value.to_be_bytes());
        out
    }

    fn example_prime() -> Vec<u8> {
        hex_vec(concat!(
            "C71CAEB9C6B1C9048E6C522F70F13F73980D40238E3E21C14934D037563D930F",
            "48198A0AA7C14058229493D22530F4DBFA336F6E0AC925139543AED44CCE7C37",
            "20FD51F69458705AC68CD4FE6B6B13ABDC9746512969328454F18FAF8C595F64",
            "2477FE96BB2A941D5BCD1D4AC8CC49880708FA9B378E3C4F3A9060BEE67CF9A4",
            "A4A695811051907E162753B56B0F6B410DBA74D8A84B2A14B3144E0EF1284754",
            "FD17ED950D5965B4B9DD46582DB1178D169C6BC465B0D6FF9CA3928FEF5B9AE4",
            "E418FC15E83EBEA0F87FA9FF5EED70050DED2849F47BF959D956850CE929851F",
            "0D8115F635B105EE2E4E15D04B2454BF6F4FADF034B10403119CD8E3B92FCC5B",
        ))
    }

    fn test_args() -> (Vec<u8>, Vec<u8>, Input) {
        (
            b"123123".to_vec(),
            hex_vec(concat!(
                "9C52401A6A8084EC82F01C3725D3FB448BD2F0C909F9D97726EAC4B7A74172D9",
                "52F02466BE6734FA274D2B7429E27397F10372D66B400B80A5C5AE3F28B17BF3",
                "105D7A2D2A885998CDC2DEFC208AEC217AB58859A9ABC2374AD93DC285F4B3FB",
                "CAFF4143D7888F2425BD2FB711B25609CEB21757D935B1EF2F042173AD0CE2FE",
                "0E474DAC53914BD25A8A9AED4AEA8953D55CB88621DB37B871EA0D04393AC098",
                "7F68094CCC9DE8239251375D8FFFD263316CD528C097B7BC9FB919FBEDB76C52",
                "5DF3413C374EE076D97A1E6D352BB7CC80FD13651B04B32E2E48C5268150842C",
                "FD07CF855958B1B5EA9C36FDAD697FE3AEC8DCC6B1EFEC36874AF226204676CF",
            )),
            Input {
                salt1: hex_vec(
                    "4D11FB6BEC38F9D2546BB0F61E4F1C99A1BC0DB8F0D5F35B1291B37B213123D7ED48F3C6794D495B",
                ),
                salt2: hex_vec("A1B181AAFE88188680AE32860D60BB01"),
                g: 3,
                p: example_prime(),
            },
        )
    }

    #[test]
    fn proof_matches_known_vector() {
        let (password, srp_b, input) = test_args();
        let answer = hash(&password, &srp_b, &set_byte(1), &input).unwrap();
        assert_eq!(
            answer,
            Answer {
                a: set_byte(3),
                m1: hex_vec("999DF906BDA2C6CBB52F503406EBA2D0D0503ACE0CC302C38F13EE5010AD4051"),
            }
        );
    }

    #[test]
    fn wrong_password_is_rejected() {
        let (_, srp_b, input) = test_args();
        let good = hash(b"123123", &srp_b, &set_byte(1), &input).unwrap();
        let bad = hash(b"123124", &srp_b, &set_byte(1), &input).unwrap();
        assert_ne!(good.m1, bad.m1);
        // A different scalar also changes the proof.
        let other = hash(b"123123", &srp_b, &set_byte(2), &input).unwrap();
        assert_ne!(good.m1, other.m1);
    }

    #[test]
    fn invalid_generator_is_rejected() {
        let (password, srp_b, mut input) = test_args();
        input.g = -1;
        assert!(hash(&password, &srp_b, &set_byte(1), &input).is_err());
    }

    #[test]
    fn new_hash_matches_known_vector() {
        let input = Input {
            salt1: vec![230, 200, 149, 125, 223, 152, 141, 72],
            salt2: vec![
                159, 99, 68, 130, 43, 9, 108, 255, 135, 239, 164, 38, 245, 120, 87, 182,
            ],
            g: 3,
            p: example_prime(),
        };

        let (hash, salt) = new_hash_with_random(b"newPassword", &input, &mut ZeroRandom).unwrap();
        assert_eq!(
            hash,
            hex_vec(concat!(
                "186ac18dcc5790bf6bba21bd958d375ee5481af0029e9bd7a9c68ec926bd5196",
                "d81f8cd8b58ee06c8a10adeacc7f56e819ff514825deb15b1fadec6aae17a244",
                "cb23488d17349cd4261a8ba4da7b9c2ce5c40014dd9e362750acf3ac89b8b8f5",
                "c618f0b6a572c38fff3a554d8818a0b8e7b6015e1836128a1e4e2d5cf9971d1d",
                "d048aa181d861152eae71553962680632387b89ac1865fded7c8c3daa64ed38d",
                "c25036663fa0cf7748c52ea19c187e70a752a8053e409d4894218a429393d033",
                "82e41e50b7415b3b8ad092fd0790f88d894e84dca78f47f4218937d7aa99d88c",
                "87c09bcb8da890e535026623cea6fc8b3d25db70cb42aaa48323927d87a8fcf1",
            ))
        );

        let mut expected_salt = input.salt1.clone();
        expected_salt.extend_from_slice(&[0u8; 32]);
        assert_eq!(salt, expected_salt);
    }
}
