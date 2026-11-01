//! Cryptographically secure randomness.
//!
//! Every random value used by this crate (authorization nonces, padding,
//! temporary RSA keys, SRP salts and Miller-Rabin witnesses) is drawn from the
//! operating system CSPRNG. Operations that accept randomness do so through the
//! [`RandomSource`] trait so tests can inject a deterministic implementation,
//! but the default and only production source is [`SystemRandom`].

/// A source of cryptographically secure random bytes.
pub trait RandomSource {
    /// Fills `dest` completely with random bytes.
    fn fill(&mut self, dest: &mut [u8]);
}

/// A [`RandomSource`] backed by the operating system CSPRNG.
#[derive(Debug, Clone, Copy)]
pub struct SystemRandom;

impl RandomSource for SystemRandom {
    fn fill(&mut self, dest: &mut [u8]) {
        fill_random(dest);
    }
}

/// Fills `dest` with bytes from the operating system CSPRNG.
///
/// # Panics
///
/// Panics if the operating system fails to provide randomness. On every
/// supported platform this is a catastrophic, unrecoverable failure.
pub fn fill_random(dest: &mut [u8]) {
    getrandom::getrandom(dest).expect("operating system CSPRNG failed");
}

/// Returns `n` freshly generated random bytes.
///
/// # Panics
///
/// Panics if the operating system fails to provide randomness.
#[must_use]
pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; n];
    fill_random(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {
    use super::{RandomSource, SystemRandom, fill_random};

    #[test]
    fn fills_entire_buffer() {
        let mut buf = [0u8; 64];
        fill_random(&mut buf);
        // The probability that 64 random bytes are all zero is negligible.
        assert_ne!(buf, [0u8; 64]);
    }

    #[test]
    fn system_random_implements_trait() {
        let mut source = SystemRandom;
        let mut buf = [0u8; 32];
        source.fill(&mut buf);
        assert_ne!(buf, [0u8; 32]);
    }
}
