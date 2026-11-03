//! Ordered update processing with gap recovery.
//!
//! [`SequenceBox`] applies updates in `pts` order, buffering those that arrive
//! ahead of the expected position and signalling when a gap has persisted long
//! enough to request a difference. It is payload-agnostic so the same logic
//! drives the pts, qts and seq streams.

use std::collections::BTreeMap;

use rtd_tg::Update;

/// A `pts`-carrying update's position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtsUpdate {
    /// The channel the update belongs to, when stated explicitly.
    pub channel_id: Option<i64>,
    /// The new `pts`.
    pub pts: i32,
    /// How many `pts` this update advances.
    pub pts_count: i32,
}

/// Returns the `pts` position of an update, if it carries one.
#[must_use]
pub fn pts_update(update: &Update) -> Option<PtsUpdate> {
    match update {
        Update::UpdateNewMessage { pts, pts_count, .. }
        | Update::UpdateEditMessage { pts, pts_count, .. }
        | Update::UpdateDeleteMessages { pts, pts_count, .. }
        | Update::UpdateNewChannelMessage { pts, pts_count, .. }
        | Update::UpdateEditChannelMessage { pts, pts_count, .. } => Some(PtsUpdate {
            channel_id: None,
            pts: *pts,
            pts_count: *pts_count,
        }),
        Update::UpdateDeleteChannelMessages {
            channel_id,
            pts,
            pts_count,
            ..
        } => Some(PtsUpdate {
            channel_id: Some(*channel_id),
            pts: *pts,
            pts_count: *pts_count,
        }),
        _ => None,
    }
}

/// Returns the `qts` position of an update, if it carries one.
#[must_use]
pub fn qts_update(update: &Update) -> Option<i32> {
    match update {
        Update::UpdateNewEncryptedMessage { qts, .. } => Some(*qts),
        _ => None,
    }
}

/// A sort key placing updates in application order.
///
/// Non-`pts` updates sort first (order preserved), then common `pts`, then
/// common `qts`, then per-channel `pts` (by channel id, then `pts`).
fn sort_key(update: &Update) -> (u8, i64, i32) {
    if let Some(pts) = pts_update(update) {
        return match pts.channel_id {
            Some(channel) => (3, channel, pts.pts - pts.pts_count),
            None => (1, 0, pts.pts - pts.pts_count),
        };
    }
    if let Some(qts) = qts_update(update) {
        return (2, 0, qts);
    }
    (0, 0, 0)
}

/// Sorts updates in the order the server expects them applied (stable).
pub fn sort_by_pts(updates: &mut [Update]) {
    updates.sort_by_key(sort_key);
}

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
    fn extracts_pts_and_sorts() {
        use rtd_tg::Update;

        let message = Update::UpdateDeleteMessages {
            messages: vec![1],
            pts: 5,
            pts_count: 1,
        };
        let other = Update::UpdatePtsChanged;
        let pts = super::pts_update(&message).expect("pts");
        assert_eq!((pts.pts, pts.pts_count), (5, 1));
        assert!(super::pts_update(&other).is_none());

        let mut list = vec![message, other];
        super::sort_by_pts(&mut list);
        assert!(matches!(list[0], Update::UpdatePtsChanged));
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
