//! Ordered update processing with gap recovery.
//!
//! [`SequenceBox`] applies updates in `pts` order, buffering those that arrive
//! ahead of the expected position and signalling when a gap has persisted long
//! enough to request a difference. It is payload-agnostic so the same logic
//! drives the pts, qts and seq streams.

use std::collections::BTreeMap;

/// The result of pushing an update into a [`SequenceBox`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushResult<T> {
    /// One or more contiguous updates can now be applied, in order.
    Applied(Vec<T>),
    /// The update was buffered until its predecessors arrive.
    Buffered,
    /// The update is a duplicate or already past.
    Duplicate,
}

/// Applies updates in sequence, buffering ahead-of-sequence ones.
#[derive(Debug, Clone)]
pub struct SequenceBox<T> {
    current: i32,
    gap_timeout_ms: u64,
    first_gap_ms: Option<u64>,
    buffer: BTreeMap<i32, T>,
}

impl<T> SequenceBox<T> {
    /// Creates a box expecting `current + 1` next.
    #[must_use]
    pub fn new(current: i32, gap_timeout_ms: u64) -> Self {
        Self {
            current,
            gap_timeout_ms,
            first_gap_ms: None,
            buffer: BTreeMap::new(),
        }
    }

    /// Returns the last applied position.
    #[must_use]
    pub fn current(&self) -> i32 {
        self.current
    }

    /// Returns the number of buffered (ahead-of-sequence) updates.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.buffer.len()
    }

    /// Pushes an update at `pts`, returning what can be applied.
    pub fn push(&mut self, pts: i32, value: T, now_ms: u64) -> PushResult<T> {
        if pts <= self.current {
            return PushResult::Duplicate;
        }
        self.buffer.insert(pts, value);
        if pts > self.current + 1 && self.first_gap_ms.is_none() {
            self.first_gap_ms = Some(now_ms);
        }

        let mut applied = Vec::new();
        while let Some(next) = self.buffer.keys().next().copied() {
            if next != self.current + 1 {
                break;
            }
            if let Some(value) = self.buffer.remove(&next) {
                self.current = next;
                applied.push(value);
                self.first_gap_ms = None;
            }
        }

        if applied.is_empty() {
            PushResult::Buffered
        } else {
            PushResult::Applied(applied)
        }
    }

    /// Reports whether the current gap has persisted past the timeout.
    #[must_use]
    pub fn gap_due(&self, now_ms: u64) -> bool {
        self.first_gap_ms
            .map(|start| now_ms.saturating_sub(start) >= self.gap_timeout_ms)
            .unwrap_or(false)
    }

    /// Resets the box to a new position after a difference, discarding buffered
    /// updates at or before `current`.
    pub fn reset(&mut self, current: i32) {
        self.current = current;
        self.buffer.retain(|pts, _| *pts > current);
        self.first_gap_ms = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{PushResult, SequenceBox};

    #[test]
    fn applies_in_order_and_buffers_ahead() {
        let mut sequence = SequenceBox::new(0, 1000);
        assert_eq!(sequence.push(2, "b", 0), PushResult::Buffered);
        assert_eq!(sequence.buffered(), 1);
        // Pushing the predecessor applies it and the buffered successor.
        assert_eq!(
            sequence.push(1, "a", 0),
            PushResult::Applied(vec!["a", "b"])
        );
        assert_eq!(sequence.current(), 2);
        assert_eq!(sequence.buffered(), 0);
        // A duplicate is ignored.
        assert_eq!(sequence.push(1, "a", 0), PushResult::Duplicate);
    }

    #[test]
    fn signals_persistent_gaps() {
        let mut sequence = SequenceBox::new(0, 1000);
        assert!(!sequence.gap_due(0));
        sequence.push(5, "e", 100);
        assert!(!sequence.gap_due(500));
        assert!(sequence.gap_due(1100));
        // A reset clears the gap.
        sequence.reset(4);
        assert!(!sequence.gap_due(1100));
    }
}
