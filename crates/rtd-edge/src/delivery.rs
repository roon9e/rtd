//! Cross-DC delivery.
//!
//! A [`Delivery`] enqueues envelopes (results or updates) for a target and
//! retains them until acknowledged, so a reconnect can replay what was not
//! delivered. [`LocalDelivery`] is an in-memory implementation.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// A unit of work delivered to a target (for example a DC label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// The monotonically increasing sequence within the target.
    pub seq: u64,
    /// The opaque, already-encoded payload.
    pub payload: Vec<u8>,
}

/// Delivers envelopes to targets with at-least-once semantics.
pub trait Delivery: Send + Sync {
    /// Enqueues `payload` for `target`, returning its sequence.
    fn enqueue(&self, target: &str, payload: Vec<u8>) -> u64;

    /// Returns every unacknowledged envelope for `target`, oldest first.
    fn drain(&self, target: &str) -> Vec<Envelope>;

    /// Marks `seq` (and everything older) acknowledged for `target`.
    fn ack(&self, target: &str, seq: u64);
}

/// An in-memory [`Delivery`] bounded per target.
#[derive(Debug)]
pub struct LocalDelivery {
    max_per_target: usize,
    next_seq: Mutex<u64>,
    queues: Mutex<HashMap<String, VecDeque<Envelope>>>,
}

impl LocalDelivery {
    /// Creates a delivery queue holding at most `max_per_target` envelopes per
    /// target (oldest are dropped when exceeded).
    #[must_use]
    pub fn new(max_per_target: usize) -> Self {
        Self {
            max_per_target: max_per_target.max(1),
            next_seq: Mutex::new(1),
            queues: Mutex::new(HashMap::new()),
        }
    }
}

impl Delivery for LocalDelivery {
    fn enqueue(&self, target: &str, payload: Vec<u8>) -> u64 {
        let mut next = self
            .next_seq
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let seq = *next;
        *next += 1;
        drop(next);

        let mut queues = self
            .queues
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let queue = queues.entry(target.to_owned()).or_default();
        queue.push_back(Envelope { seq, payload });
        while queue.len() > self.max_per_target {
            queue.pop_front();
        }
        seq
    }

    fn drain(&self, target: &str) -> Vec<Envelope> {
        self.queues
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(target)
            .map(|queue| queue.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn ack(&self, target: &str, seq: u64) {
        let mut queues = self
            .queues
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(queue) = queues.get_mut(target) {
            queue.retain(|envelope| envelope.seq > seq);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Delivery, LocalDelivery};

    #[test]
    fn retains_until_acknowledged_and_replays() {
        let delivery = LocalDelivery::new(10);
        let first = delivery.enqueue("dc2", b"one".to_vec());
        let second = delivery.enqueue("dc2", b"two".to_vec());
        assert_eq!(delivery.drain("dc2").len(), 2);

        // Acknowledge the first; the second is replayed on the next drain.
        delivery.ack("dc2", first);
        let remaining = delivery.drain("dc2");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].seq, second);
    }

    #[test]
    fn retention_is_bounded() {
        let delivery = LocalDelivery::new(2);
        delivery.enqueue("dc2", b"a".to_vec());
        delivery.enqueue("dc2", b"b".to_vec());
        delivery.enqueue("dc2", b"c".to_vec());
        let remaining = delivery.drain("dc2");
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].payload, b"b");
    }
}
