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
        }
    }

    /// Returns a ping id to send when a ping is due, or `None`.
    ///
    /// A ping is not due while one is still awaiting its `pong`.
    pub fn due(&self, now_ms: u64, ping_id: i64) -> Option<i64> {
        if self.pending.is_some() || now_ms < self.next_ping_ms {
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
}
