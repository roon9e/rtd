//! Injectable randomness.
//!
//! The canonical trait and its implementations live in `rtd-util`; this module
//! re-exports them under the crate's historical names so that protocol code and
//! tests share a single injectable source. [`RandomSource`] is the production
//! operating-system CSPRNG; tests inject `rtd_util::DeterministicRandom`.

pub use rtd_util::random::{Random as RandomSource, SystemRandom, fill_random, random_bytes};
