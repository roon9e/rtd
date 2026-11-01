//! Injectable time.
//!
//! The sans-IO protocol core must not read the wall clock directly: message
//! identifiers, salts and timeouts all depend on time, and tests need to
//! advance it without waiting. Protocol code therefore takes a [`Clock`].

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A source of the current time.
pub trait Clock {
    /// Returns the current time.
    fn now(&self) -> SystemTime;

    /// Returns the current time as nanoseconds since the Unix epoch.
    ///
    /// Times before the epoch are negative.
    fn unix_nanos(&self) -> i64 {
        match self.now().duration_since(UNIX_EPOCH) {
            Ok(since) => since.as_nanos() as i64,
            Err(before) => -(before.duration().as_nanos() as i64),
        }
    }
}

/// A [`Clock`] backed by the operating system wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// A [`Clock`] whose time is advanced explicitly, for tests.
///
/// Clones share the same underlying time.
#[derive(Debug, Clone)]
pub struct ManualClock {
    now: Arc<Mutex<SystemTime>>,
}

impl ManualClock {
    /// Creates a clock starting at `start`.
    #[must_use]
    pub fn new(start: SystemTime) -> Self {
        Self {
            now: Arc::new(Mutex::new(start)),
        }
    }

    /// Creates a clock starting at `UNIX_EPOCH`.
    #[must_use]
    pub fn from_epoch() -> Self {
        Self::new(UNIX_EPOCH)
    }

    /// Advances the clock by `delta`.
    ///
    /// A caller holding the lock while panicking leaves it poisoned; the clock
    /// is a test utility whose value stays usable, so poisoning is recovered
    /// rather than propagated.
    pub fn advance(&self, delta: Duration) {
        let mut now = self.lock();
        *now = now.checked_add(delta).unwrap_or(*now);
    }

    /// Sets the clock to `time`.
    pub fn set(&self, time: SystemTime) {
        *self.lock() = time;
    }

    /// Locks the shared time, recovering from poisoning.
    fn lock(&self) -> MutexGuard<'_, SystemTime> {
        self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::from_epoch()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.lock()
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, ManualClock, SystemClock};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn manual_clock_advances_without_waiting() {
        let clock = ManualClock::from_epoch();
        assert_eq!(clock.unix_nanos(), 0);
        clock.advance(Duration::from_secs(5));
        assert_eq!(clock.unix_nanos(), 5_000_000_000);
        clock.advance(Duration::from_millis(1));
        assert_eq!(clock.unix_nanos(), 5_001_000_000);
    }

    #[test]
    fn manual_clock_clones_share_time() {
        let clock = ManualClock::new(UNIX_EPOCH);
        let observer = clock.clone();
        clock.advance(Duration::from_secs(2));
        assert_eq!(observer.now(), UNIX_EPOCH + Duration::from_secs(2));
    }

    #[test]
    fn system_clock_is_after_epoch() {
        assert!(SystemClock.unix_nanos() > 0);
    }
}
