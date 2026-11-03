//! Ordered update processing with gap recovery.
//!
//! [`SequenceBox`] applies updates in `pts` order, buffering those that arrive
//! ahead of the expected position and signalling when a gap has persisted long
//! enough to request a difference. It is payload-agnostic so the same logic
//! drives the pts, qts and seq streams.

use std::collections::BTreeMap;

use rtd_tg::{
    ChannelMessagesFilter, InputChannel, Message, Method, Object, Update, UpdatesChannelDifference,
    UpdatesDifference, UpdatesGetChannelDifference, UpdatesGetDifference,
};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// The persisted update cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    /// The `pts` cursor.
    pub pts: i32,
    /// The `qts` cursor.
    pub qts: i32,
    /// The `date` cursor.
    pub date: i32,
    /// The `seq` cursor.
    pub seq: i32,
}

/// Persists the update cursor between sessions.
pub trait StateStorage: Send + Sync {
    /// Loads the stored state, if any.
    fn load(&self) -> Option<State>;
    /// Stores `state`.
    fn store(&self, state: &State);
    /// Clears the stored state.
    fn clear(&self);
}

/// An in-memory [`StateStorage`].
#[derive(Debug, Default)]
pub struct MemoryStateStorage {
    state: std::sync::Mutex<Option<State>>,
}

impl MemoryStateStorage {
    /// Creates an empty storage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl StateStorage for MemoryStateStorage {
    fn load(&self) -> Option<State> {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn store(&self, state: &State) {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(*state);
    }

    fn clear(&self) {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

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

/// The recovered difference between the local state and the server.
#[derive(Debug, Clone, Default)]
pub struct Difference {
    /// The recovered updates, sorted for application.
    pub updates: Vec<Update>,
    /// The messages referenced by the updates.
    pub messages: Vec<Message>,
    /// The `(pts, qts, date)` to advance to.
    pub state: (i32, i32, i32),
    /// Whether the server's history was too long to recover incrementally.
    pub too_long: bool,
}

/// Fetches `updates.getDifference`, draining `DifferenceSlice` pages.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
/// when the server answers with an unexpected constructor.
pub fn fetch_difference<T: Transport>(
    client: &mut Client<T>,
    pts: i32,
    qts: i32,
    date: i32,
) -> Result<Difference> {
    let mut difference = Difference {
        state: (pts, qts, date),
        ..Difference::default()
    };
    loop {
        let query = Method::UpdatesGetDifference(Box::new(UpdatesGetDifference {
            flags: 0,
            pts: difference.state.0,
            pts_limit: None,
            pts_total_limit: None,
            date: difference.state.2,
            qts: difference.state.1,
            qts_limit: None,
        }));
        match client.invoke(query)? {
            Object::UpdatesDifference(page) => match *page {
                UpdatesDifference::Difference {
                    new_messages,
                    other_updates,
                    state,
                    ..
                } => {
                    difference.messages.extend(new_messages);
                    difference.updates.extend(other_updates);
                    difference.state = (state.pts, state.qts, state.date);
                    break;
                }
                UpdatesDifference::DifferenceSlice {
                    new_messages,
                    other_updates,
                    intermediate_state,
                    ..
                } => {
                    difference.messages.extend(new_messages);
                    difference.updates.extend(other_updates);
                    difference.state = (
                        intermediate_state.pts,
                        intermediate_state.qts,
                        intermediate_state.date,
                    );
                }
                UpdatesDifference::DifferenceEmpty { date, .. } => {
                    difference.state.2 = date;
                    break;
                }
                UpdatesDifference::DifferenceTooLong { pts } => {
                    difference.state.0 = pts;
                    difference.too_long = true;
                    break;
                }
            },
            other => {
                return Err(Error::Unexpected(format!(
                    "updates.getDifference returned unexpected constructor {:#010x}",
                    other.type_id()
                )));
            }
        }
    }
    sort_by_pts(&mut difference.updates);
    Ok(difference)
}

/// The recovered difference for one channel.
#[derive(Debug, Clone, Default)]
pub struct ChannelDifference {
    /// The recovered updates, sorted for application.
    pub updates: Vec<Update>,
    /// The messages referenced by the updates.
    pub messages: Vec<Message>,
    /// The channel `pts` to advance to.
    pub pts: i32,
    /// Whether the server's history was too long to recover incrementally.
    pub too_long: bool,
}

/// Fetches `updates.getChannelDifference` until the server reports final.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
/// when the server answers with an unexpected constructor.
pub fn fetch_channel_difference<T: Transport>(
    client: &mut Client<T>,
    channel: InputChannel,
    pts: i32,
) -> Result<ChannelDifference> {
    let mut difference = ChannelDifference {
        pts,
        ..ChannelDifference::default()
    };
    loop {
        let query = Method::UpdatesGetChannelDifference(Box::new(UpdatesGetChannelDifference {
            flags: 0,
            force: false,
            channel: Box::new(channel.clone()),
            filter: Box::new(ChannelMessagesFilter::ChannelMessagesFilterEmpty),
            pts: difference.pts,
            limit: 100,
        }));
        match client.invoke(query)? {
            Object::UpdatesChannelDifference(page) => match *page {
                UpdatesChannelDifference::ChannelDifference {
                    final_,
                    pts,
                    new_messages,
                    other_updates,
                    ..
                } => {
                    difference.messages.extend(new_messages);
                    difference.updates.extend(other_updates);
                    difference.pts = pts;
                    if final_ {
                        break;
                    }
                }
                UpdatesChannelDifference::ChannelDifferenceEmpty { final_, pts, .. } => {
                    difference.pts = pts;
                    if final_ {
                        break;
                    }
                }
                UpdatesChannelDifference::ChannelDifferenceTooLong { .. } => {
                    difference.too_long = true;
                    break;
                }
            },
            other => {
                return Err(Error::Unexpected(format!(
                    "updates.getChannelDifference returned unexpected constructor {:#010x}",
                    other.type_id()
                )));
            }
        }
    }
    sort_by_pts(&mut difference.updates);
    Ok(difference)
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

/// A `pts` box that advances across multi-count updates.
///
/// Updates are keyed by their ending `pts`; an update is applicable once its
/// covered range `(pts - pts_count, pts]` begins at or before the current
/// position, so a single update may advance the position by more than one.
#[derive(Debug, Clone)]
struct PtsBox {
    current: i32,
    buffer: BTreeMap<i32, (i32, Update)>,
    first_gap_ms: Option<u64>,
}

/// The result of pushing an update into a [`PtsBox`].
enum PtsOutcome {
    /// A duplicate or already-applied update.
    Duplicate,
    /// Buffered ahead of a gap.
    Buffered,
    /// One or more contiguous updates to apply.
    Applied(Vec<Update>),
}

impl PtsBox {
    fn new(current: i32) -> Self {
        Self {
            current,
            buffer: BTreeMap::new(),
            first_gap_ms: None,
        }
    }

    fn push(
        &mut self,
        pts: i32,
        pts_count: i32,
        update: Update,
        now_ms: u64,
        timeout_ms: u64,
    ) -> PtsOutcome {
        let _ = timeout_ms;
        if pts <= self.current {
            return PtsOutcome::Duplicate;
        }
        self.buffer.insert(pts, (pts_count, update));
        if pts - pts_count > self.current && self.first_gap_ms.is_none() {
            self.first_gap_ms = Some(now_ms);
        }

        let mut applied = Vec::new();
        loop {
            let key = self
                .buffer
                .iter()
                .find(|(end, (count, _))| **end - *count <= self.current)
                .map(|(end, _)| *end);
            let Some(key) = key else { break };
            if let Some((_, update)) = self.buffer.remove(&key) {
                if key > self.current {
                    self.current = key;
                }
                applied.push(update);
            }
        }
        if self.buffer.is_empty() {
            self.first_gap_ms = None;
        }

        if applied.is_empty() {
            PtsOutcome::Buffered
        } else {
            PtsOutcome::Applied(applied)
        }
    }

    fn reset(&mut self, current: i32) {
        self.current = current;
        self.buffer.retain(|end, _| *end > current);
        self.first_gap_ms = None;
    }

    fn gap_due(&self, now_ms: u64, timeout_ms: u64) -> bool {
        !self.buffer.is_empty()
            && self
                .first_gap_ms
                .map(|start| now_ms.saturating_sub(start) >= timeout_ms)
                .unwrap_or(false)
    }
}

/// Callbacks the [`Manager`] invokes on recovery events.
#[derive(Default)]
pub struct Callbacks {
    /// Called when the server's history is too long to recover incrementally.
    pub on_too_long: Option<Box<dyn FnMut() + Send>>,
    /// Called when stored update state could not be loaded.
    pub on_load_failed: Option<Box<dyn FnMut() + Send>>,
}

/// What the [`Manager`] asks the caller to do after an update.
#[derive(Debug)]
pub enum Action {
    /// Apply these updates, in order.
    Apply(Vec<Update>),
    /// The update was buffered ahead of a gap.
    Buffered,
    /// The update was a duplicate or already applied.
    Ignored,
    /// A common gap has persisted: request `updates.getDifference`.
    NeedDifference,
    /// A channel gap has persisted: request `updates.getChannelDifference`.
    NeedChannelDifference {
        /// The channel with the gap.
        channel_id: i64,
        /// The channel `pts` to recover from.
        pts: i32,
    },
}

/// Wires the common, per-channel and state boxes together.
///
/// The manager is transport-agnostic: the caller feeds updates in and reacts to
/// the returned [`Action`], fetching differences when asked.
pub struct Manager {
    state: State,
    gap_timeout_ms: u64,
    common: PtsBox,
    channels: BTreeMap<i64, PtsBox>,
    storage: Option<Box<dyn StateStorage>>,
    callbacks: Callbacks,
}

impl Manager {
    /// Creates a manager positioned at `state`, with a gap timeout in
    /// milliseconds.
    #[must_use]
    pub fn new(state: State, gap_timeout_ms: u64) -> Self {
        Self {
            state,
            gap_timeout_ms,
            common: PtsBox::new(state.pts),
            channels: BTreeMap::new(),
            storage: None,
            callbacks: Callbacks::default(),
        }
    }

    /// Attaches a state storage, loading any stored cursor immediately.
    #[must_use]
    pub fn with_storage(mut self, storage: Box<dyn StateStorage>) -> Self {
        if let Some(state) = storage.load() {
            self.state = state;
            self.common.reset(state.pts);
        } else if let Some(callback) = &mut self.callbacks.on_load_failed {
            callback();
        }
        self.storage = Some(storage);
        self
    }

    /// Returns mutable access to the callbacks.
    pub fn callbacks(&mut self) -> &mut Callbacks {
        &mut self.callbacks
    }

    /// Returns the current cursor.
    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// Feeds an update into the manager.
    pub fn push(&mut self, update: Update, now_ms: u64) -> Action {
        let Some(pts) = pts_update(&update) else {
            return Action::Apply(vec![update]);
        };
        let outcome = match pts.channel_id {
            Some(channel_id) => {
                let current = self
                    .channels
                    .get(&channel_id)
                    .map_or(self.state.pts, |box_| box_.current);
                let box_ = self
                    .channels
                    .entry(channel_id)
                    .or_insert_with(|| PtsBox::new(current));
                box_.push(pts.pts, pts.pts_count, update, now_ms, self.gap_timeout_ms)
            }
            None => self
                .common
                .push(pts.pts, pts.pts_count, update, now_ms, self.gap_timeout_ms),
        };
        match outcome {
            PtsOutcome::Duplicate => Action::Ignored,
            PtsOutcome::Buffered => Action::Buffered,
            PtsOutcome::Applied(updates) => {
                if pts.channel_id.is_none() {
                    self.state.pts = self.common.current;
                    self.persist();
                }
                Action::Apply(updates)
            }
        }
    }

    /// Reports a due gap, if any, without draining new updates.
    #[must_use]
    pub fn poll(&self, now_ms: u64) -> Option<Action> {
        if self.common.gap_due(now_ms, self.gap_timeout_ms) {
            return Some(Action::NeedDifference);
        }
        for (channel_id, box_) in &self.channels {
            if box_.gap_due(now_ms, self.gap_timeout_ms) {
                return Some(Action::NeedChannelDifference {
                    channel_id: *channel_id,
                    pts: box_.current,
                });
            }
        }
        None
    }

    /// Applies a recovered common difference, resetting the common box.
    pub fn apply_difference(&mut self, difference: Difference) -> Vec<Update> {
        if difference.too_long
            && let Some(callback) = &mut self.callbacks.on_too_long
        {
            callback();
        }
        let (pts, qts, date) = difference.state;
        self.state.pts = pts;
        self.state.qts = qts;
        self.state.date = date;
        self.common.reset(pts);
        self.persist();
        difference.updates
    }

    /// Applies a recovered channel difference, resetting that channel's box.
    pub fn apply_channel_difference(
        &mut self,
        channel_id: i64,
        difference: ChannelDifference,
    ) -> Vec<Update> {
        let box_ = self
            .channels
            .entry(channel_id)
            .or_insert_with(|| PtsBox::new(0));
        box_.reset(difference.pts);
        difference.updates
    }

    /// Persists the current cursor to the attached storage, if any.
    fn persist(&self) {
        if let Some(storage) = &self.storage {
            storage.store(&self.state);
        }
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
    fn state_storage_round_trips() {
        use super::{MemoryStateStorage, State, StateStorage};
        let storage = MemoryStateStorage::new();
        assert!(storage.load().is_none());
        storage.store(&State {
            pts: 5,
            qts: 1,
            date: 3,
            seq: 2,
        });
        assert_eq!(storage.load().map(|s| s.pts), Some(5));
        storage.clear();
        assert!(storage.load().is_none());
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

    #[test]
    fn manager_orders_updates_and_recovers_gaps() {
        use rtd_tg::Update;

        use super::{Action, Difference, Manager, State};

        let make = |pts| Update::UpdateDeleteMessages {
            messages: vec![pts],
            pts,
            pts_count: 1,
        };
        let mut manager = Manager::new(State::default(), 1000);
        assert!(matches!(manager.push(make(2), 0), Action::Buffered));
        match manager.push(make(1), 0) {
            Action::Apply(updates) => assert_eq!(updates.len(), 2),
            other => panic!("expected apply, got {other:?}"),
        }
        assert_eq!(manager.state().pts, 2);
        assert!(matches!(manager.push(make(1), 0), Action::Ignored));

        assert!(matches!(manager.push(make(5), 100), Action::Buffered));
        assert!(manager.poll(500).is_none());
        assert!(matches!(manager.poll(1100), Some(Action::NeedDifference)));

        let difference = Difference {
            updates: vec![make(5)],
            state: (5, 0, 0),
            ..Difference::default()
        };
        assert_eq!(manager.apply_difference(difference).len(), 1);
        assert_eq!(manager.state().pts, 5);
        assert!(manager.poll(2000).is_none());
    }

    #[test]
    fn manager_tracks_channel_gaps_and_storage() {
        use rtd_tg::Update;

        use super::{Action, ChannelDifference, Manager, MemoryStateStorage, State, StateStorage};

        let storage = MemoryStateStorage::new();
        storage.store(&State {
            pts: 10,
            ..State::default()
        });
        let mut manager = Manager::new(State::default(), 1000).with_storage(Box::new(storage));
        // The stored cursor is loaded.
        assert_eq!(manager.state().pts, 10);

        let update = Update::UpdateDeleteChannelMessages {
            channel_id: 7,
            messages: vec![1],
            pts: 15,
            pts_count: 1,
        };
        assert!(matches!(manager.push(update, 0), Action::Buffered));
        match manager.poll(1100) {
            Some(Action::NeedChannelDifference { channel_id, pts }) => {
                assert_eq!(channel_id, 7);
                assert_eq!(pts, 10);
            }
            other => panic!("expected channel difference, got {other:?}"),
        }
        let difference = ChannelDifference {
            updates: vec![],
            pts: 15,
            ..ChannelDifference::default()
        };
        assert!(manager.apply_channel_difference(7, difference).is_empty());
        assert!(manager.poll(2000).is_none());
    }
}
