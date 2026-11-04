//! Keepalive scheduling.
//!
//! [`Keepalive`] decides when a `ping` is due and whether a missing `pong` means
//! the connection is dead. Time enters as monotonic milliseconds, so the logic
//! is deterministic and the caller drives the actual send.

use std::time::Duration;

/// Decides when to ping and detects a dead connection.
#[derive(Debug, Clone)]
pub struct Keepalive {
    interval_ms: u64,
    timeout_ms: u64,
    next_ping_ms: u64,
    pending: Option<(i64, u64)>,
    dead: bool,
}

impl Keepalive {
    /// Creates a scheduler that pings every `interval` and expects a `pong`
    /// within `timeout`, the first ping due at `start_ms + interval`.
    #[must_use]
    pub fn new(interval: Duration, timeout: Duration, start_ms: u64) -> Self {
        let interval_ms = interval.as_millis() as u64;
        Self {
            interval_ms,
            timeout_ms: timeout.as_millis() as u64,
            next_ping_ms: start_ms.saturating_add(interval_ms),
            pending: None,
            dead: false,
        }
    }

    /// Returns a ping id to send when a ping is due, or `None`.
    ///
    /// A ping is not due while one is still awaiting its `pong` or once the
    /// connection has been marked dead.
    pub fn due(&self, now_ms: u64, ping_id: i64) -> Option<i64> {
        if self.dead || self.pending.is_some() || now_ms < self.next_ping_ms {
            None
        } else {
            Some(ping_id)
        }
    }

    /// Records that a ping was sent at `now_ms`.
    pub fn sent(&mut self, ping_id: i64, now_ms: u64) {
        self.pending = Some((ping_id, now_ms));
    }

    /// Records a `pong`, scheduling the next ping. Returns whether it matched.
    pub fn on_pong(&mut self, ping_id: i64, now_ms: u64) -> bool {
        match self.pending {
            Some((pending_id, _)) if pending_id == ping_id => {
                self.pending = None;
                self.next_ping_ms = now_ms.saturating_add(self.interval_ms);
                true
            }
            _ => false,
        }
    }

    /// Reports whether the pending ping has exceeded the timeout.
    #[must_use]
    pub fn timed_out(&self, now_ms: u64) -> bool {
        self.pending
            .map(|(_, sent_at)| now_ms.saturating_sub(sent_at) >= self.timeout_ms)
            .unwrap_or(false)
    }

    /// Marks the connection dead if the pending ping has timed out.
    ///
    /// Returns whether the connection is dead. Once dead it stays dead until
    /// [`Keepalive::reconnect`].
    pub fn check(&mut self, now_ms: u64) -> bool {
        if self.timed_out(now_ms) {
            self.dead = true;
        }
        self.dead
    }

    /// Returns whether the connection has been marked dead.
    #[must_use]
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Clears the dead state and pending ping after a reconnect, scheduling the
    /// first post-reconnect ping.
    pub fn reconnect(&mut self, now_ms: u64) {
        self.pending = None;
        self.dead = false;
        self.next_ping_ms = now_ms.saturating_add(self.interval_ms);
    }
}

/// Capped exponential backoff, in monotonic milliseconds.
#[derive(Debug, Clone)]
pub struct Backoff {
    initial_ms: u64,
    max_ms: u64,
    factor: u64,
    current_ms: u64,
}

impl Backoff {
    /// Creates a backoff starting at `initial` and doubling until `max`.
    #[must_use]
    pub fn new(initial: Duration, max: Duration) -> Self {
        let initial_ms = initial.as_millis() as u64;
        let max_ms = (max.as_millis() as u64).max(initial_ms);
        Self {
            initial_ms,
            max_ms,
            factor: 2,
            current_ms: initial_ms,
        }
    }

    /// Overrides the growth factor (values below 2 are raised to 2).
    #[must_use]
    pub fn with_factor(mut self, factor: u64) -> Self {
        self.factor = factor.max(2);
        self
    }

    /// Returns the current delay and advances to the next (capped) delay.
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current_ms;
        self.current_ms = self.current_ms.saturating_mul(self.factor).min(self.max_ms);
        Duration::from_millis(delay)
    }

    /// Resets the backoff to its initial delay.
    pub fn reset(&mut self) {
        self.current_ms = self.initial_ms;
    }
}

/// A reconnect plan that reuses an existing session with capped backoff.
#[derive(Debug, Clone)]
pub struct Reconnect {
    session: crate::session::SessionData,
    backoff: Backoff,
}

impl Reconnect {
    /// Creates a plan resuming `session`, backing off from `initial` to `max`.
    #[must_use]
    pub fn new(session: crate::session::SessionData, initial: Duration, max: Duration) -> Self {
        Self {
            session,
            backoff: Backoff::new(initial, max),
        }
    }

    /// Returns the session to resume, preserving the authorization key and salt.
    #[must_use]
    pub fn session(&self) -> &crate::session::SessionData {
        &self.session
    }

    /// Returns the next reconnect delay and advances the backoff.
    pub fn next_delay(&mut self) -> Duration {
        self.backoff.next_delay()
    }

    /// Resets the backoff after a successful reconnect.
    pub fn reset(&mut self) {
        self.backoff.reset();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Keepalive;

    #[test]
    fn pings_on_interval_and_detects_timeout() {
        let mut keepalive = Keepalive::new(Duration::from_secs(60), Duration::from_secs(5), 0);
        // Not due before the interval.
        assert_eq!(keepalive.due(59_000, 1), None);
        assert_eq!(keepalive.due(60_000, 1), Some(1));

        keepalive.sent(1, 60_000);
        // While awaiting, no new ping is due.
        assert_eq!(keepalive.due(120_000, 2), None);
        assert!(!keepalive.timed_out(64_000));
        assert!(keepalive.timed_out(65_000));

        // A matching pong schedules the next ping.
        assert!(keepalive.on_pong(1, 65_000));
        assert_eq!(keepalive.due(124_000, 2), None);
        assert_eq!(keepalive.due(125_000, 2), Some(2));
    }

    #[test]
    fn missed_pong_marks_the_connection_dead_until_reconnect() {
        let mut keepalive = Keepalive::new(Duration::from_secs(60), Duration::from_secs(5), 0);
        keepalive.sent(1, 0);
        assert!(!keepalive.check(4_000));
        assert!(keepalive.check(5_000));
        assert!(keepalive.is_dead());
        // A dead connection schedules no further pings.
        assert_eq!(keepalive.due(1_000_000, 2), None);
        keepalive.reconnect(10_000);
        assert!(!keepalive.is_dead());
        assert_eq!(keepalive.due(70_000, 3), Some(3));
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        use super::Backoff;

        let mut backoff = Backoff::new(Duration::from_millis(100), Duration::from_millis(800));
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
        assert_eq!(backoff.next_delay(), Duration::from_millis(200));
        assert_eq!(backoff.next_delay(), Duration::from_millis(400));
        assert_eq!(backoff.next_delay(), Duration::from_millis(800));
        // Capped.
        assert_eq!(backoff.next_delay(), Duration::from_millis(800));
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn reconnect_reuses_the_session_with_backoff() {
        use super::Reconnect;
        use crate::session::SessionData;

        let session = SessionData {
            auth_key: [7u8; 256],
            session_id: 5,
            salt: 9,
        };
        let mut reconnect = Reconnect::new(
            session,
            Duration::from_millis(100),
            Duration::from_millis(400),
        );
        assert_eq!(reconnect.session().session_id, 5);
        assert_eq!(reconnect.session().salt, 9);
        assert_eq!(reconnect.next_delay(), Duration::from_millis(100));
        assert_eq!(reconnect.next_delay(), Duration::from_millis(200));
        assert_eq!(reconnect.next_delay(), Duration::from_millis(400));
        // Capped at the maximum.
        assert_eq!(reconnect.next_delay(), Duration::from_millis(400));
        reconnect.reset();
        assert_eq!(reconnect.next_delay(), Duration::from_millis(100));
    }
}
