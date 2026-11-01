//! Injectable randomness.
//!
//! The sans-IO protocol core must not call the operating system CSPRNG
//! directly when determinism is required (tests, fuzzing). Protocol code
//! therefore takes a [`Random`]; [`SystemRandom`] is the production source and
//! [`DeterministicRandom`] is a reproducible source for tests.

/// A source of random bytes.
pub trait Random {
    /// Fills `dest` completely with random bytes.
    fn fill(&mut self, dest: &mut [u8]);
}

/// A [`Random`] backed by the operating system CSPRNG.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRandom;

impl Random for SystemRandom {
    fn fill(&mut self, dest: &mut [u8]) {
        fill_random(dest);
    }
}

/// A reproducible, non-cryptographic [`Random`] for tests and fuzzing.
///
/// This is a xorshift64* generator. It MUST NOT be used for key material or
/// any security-relevant value.
#[derive(Debug, Clone)]
pub struct DeterministicRandom {
    state: u64,
}

impl DeterministicRandom {
    /// Creates a generator from `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        // A zero state is a fixed point of xorshift; force it non-zero.
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    /// Returns the next 64 bits of output.
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

impl Random for DeterministicRandom {
    fn fill(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let word = self.next_u64().to_le_bytes();
            let n = chunk.len();
            chunk.copy_from_slice(&word[..n]);
        }
    }
}

/// Fills `dest` with bytes from the operating system CSPRNG.
///
/// # Panics
///
/// Panics if the operating system fails to provide randomness; on every
/// supported platform this is a catastrophic, unrecoverable failure.
pub fn fill_random(dest: &mut [u8]) {
    getrandom::fill(dest).expect("operating system CSPRNG failed");
}

/// Returns `n` freshly generated random bytes from the operating system CSPRNG.
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
    use super::{DeterministicRandom, Random, SystemRandom, fill_random};

    #[test]
    fn deterministic_source_is_reproducible() {
        let mut a = DeterministicRandom::new(42);
        let mut b = DeterministicRandom::new(42);
        let mut first = [0u8; 37];
        let mut second = [0u8; 37];
        a.fill(&mut first);
        b.fill(&mut second);
        assert_eq!(first, second);
        assert_ne!(first, [0u8; 37]);
    }

    #[test]
    fn deterministic_sources_differ_by_seed() {
        let mut a = DeterministicRandom::new(1);
        let mut b = DeterministicRandom::new(2);
        let (mut x, mut y) = ([0u8; 32], [0u8; 32]);
        a.fill(&mut x);
        b.fill(&mut y);
        assert_ne!(x, y);
    }

    #[test]
    fn system_random_fills_buffer() {
        let mut source = SystemRandom;
        let mut buf = [0u8; 64];
        source.fill(&mut buf);
        assert_ne!(buf, [0u8; 64]);
        let mut other = [0u8; 8];
        fill_random(&mut other);
    }
}
