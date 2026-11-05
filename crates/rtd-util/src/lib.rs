//! Shared utilities for the sans-IO protocol core.
#![warn(missing_docs)]
//!
//! The protocol crates own no sockets and no runtime, and they must not read
//! ambient time or randomness. This crate provides the injectable [`Clock`] and
//! [`Random`] sources they take instead, plus production ([`SystemClock`],
//! [`SystemRandom`]) and test ([`ManualClock`], [`DeterministicRandom`])
//! implementations.

pub mod clock;
#[cfg(feature = "ntp")]
pub mod ntp;
pub mod random;

pub use clock::{Clock, ManualClock, SystemClock};
#[cfg(feature = "ntp")]
pub use ntp::{NtpClock, NtpOffset};
pub use random::{DeterministicRandom, Random, SystemRandom, fill_random, random_bytes};
