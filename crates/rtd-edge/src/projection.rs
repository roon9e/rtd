//! Update projection with per-client watermarks.
//!
//! [`Projection`] tracks the highest `pts` delivered and acknowledged per
//! client (and per channel), so an in-order delivery can be enforced and a
//! reconnect can resume from the last confirmed position.

use std::collections::HashMap;
use std::sync::Mutex;

/// A monotonic watermark pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Watermark {
    /// The highest `pts` delivered.
    pub delivered: i32,
    /// The highest `pts` acknowledged.
    pub acknowledged: i32,
}

/// Tracks per-client and per-channel watermarks.
#[derive(Debug, Default)]
pub struct Projection {
    clients: Mutex<HashMap<String, Watermark>>,
    channels: Mutex<HashMap<String, Watermark>>,
}

impl Projection {
    /// Creates an empty projection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a delivered `pts` for `client`.
    ///
    /// Returns `false` when the `pts` is not newer than what was delivered
    /// (out of order or a duplicate), so the caller can buffer it.
    pub fn deliver(&self, client: &str, pts: i32) -> bool {
        Self::advance(&self.clients, client, pts)
    }

    /// Records an acknowledged `pts` for `client`.
    pub fn acknowledge(&self, client: &str, pts: i32) {
        let mut clients = self
            .clients
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(watermark) = clients.get_mut(client) {
            watermark.acknowledged = watermark.acknowledged.max(pts);
        }
    }

    /// Records a delivered `pts` for a channel.
    pub fn deliver_channel(&self, channel: &str, pts: i32) -> bool {
        Self::advance(&self.channels, channel, pts)
    }

    /// Returns the watermark to resume from after a reconnect.
    #[must_use]
    pub fn resume(&self, client: &str) -> i32 {
        self.clients
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(client)
            .map(|watermark| watermark.acknowledged)
            .unwrap_or(0)
    }

    /// Returns the current watermark for `client`.
    #[must_use]
    pub fn watermark(&self, client: &str) -> Watermark {
        self.clients
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(client)
            .copied()
            .unwrap_or_default()
    }

    /// Advances a watermark, returning whether `pts` was newer.
    fn advance(map: &Mutex<HashMap<String, Watermark>>, key: &str, pts: i32) -> bool {
        let mut map = map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let watermark = map.entry(key.to_owned()).or_default();
        if pts > watermark.delivered {
            watermark.delivered = pts;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Projection;

    #[test]
    fn enforces_order_and_resumes_from_acknowledged() {
        let projection = Projection::new();
        assert!(projection.deliver("c", 1));
        assert!(projection.deliver("c", 2));
        // A duplicate or out-of-order update is not applied.
        assert!(!projection.deliver("c", 2));

        projection.acknowledge("c", 2);
        assert_eq!(projection.resume("c"), 2);
        assert_eq!(projection.watermark("c").delivered, 2);
    }

    #[test]
    fn channels_track_independently() {
        let projection = Projection::new();
        assert!(projection.deliver_channel("ch", 5));
        assert!(!projection.deliver_channel("ch", 4));
        // Client and channel watermarks are separate.
        assert_eq!(projection.watermark("ch").delivered, 0);
    }
}
