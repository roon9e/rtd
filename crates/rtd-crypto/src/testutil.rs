//! Deterministic random sources used by the crate's unit tests.
//!
//! These are compiled only for tests and exist to make the otherwise random
//! primitives reproducible against known vectors.

use crate::rand::RandomSource;

/// A random source that always yields zero bytes.
pub struct ZeroRandom;

impl RandomSource for ZeroRandom {
    fn fill(&mut self, dest: &mut [u8]) {
        dest.fill(0);
    }
}

/// A random source that always yields the same byte value.
pub struct RepeatRandom {
    /// The byte every requested position is filled with.
    pub byte: u8,
}

impl RandomSource for RepeatRandom {
    fn fill(&mut self, dest: &mut [u8]) {
        dest.fill(self.byte);
    }
}

/// A small, deterministic xorshift64 pseudo-random generator.
///
/// This is used only to make randomized algorithms repeatable inside tests; it
/// is never used for anything security relevant.
pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    /// Creates a generator seeded with `seed` (zero is remapped to a constant).
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

impl RandomSource for XorShift64 {
    fn fill(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&bytes[..len]);
        }
    }
}
