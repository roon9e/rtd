//! Sans-IO MTProto session state machine.
//!
//! A [`Session`] owns the per-connection protocol state that is independent of
//! any socket or runtime: message-identifier and sequence-number allocation, the
//! inbound replay window, the set of outbound content messages awaiting an
//! acknowledgement (which doubles as the resend source), the acknowledgement
//! backlog for inbound messages, and the session identity and lifecycle.
//!
//! Everything time-dependent is read from an injected [`Clock`] and the session
//! identifier is drawn from an injected [`Random`]; the module never reads the
//! operating-system clock or CSPRNG and never touches the network, so the whole
//! state machine is deterministic and unit-testable with
//! [`rtd_util::ManualClock`] and [`rtd_util::DeterministicRandom`].
//!
//! The encrypted-message envelope itself lives in [`rtd_crypto`]; this module
//! consumes its inner header ([`MessageHeader`]) and never performs
//! cryptography. Replay protection and sequence allocation follow the MTProto
//! rules:
//!
//! - a content message takes sequence number `2*N+1` and increments `N`, the
//!   number of content messages allocated so far, while a service message takes
//!   `2*N` and leaves `N` unchanged;
//! - an inbound message is rejected when it is too old, too far in the future,
//!   or below the whole replay window, but a duplicate retransmission is
//!   tolerated (acknowledged without being processed again).
//!
//! See <https://core.telegram.org/mtproto/description>.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rtd_crypto::encrypted_message::MessageHeader;
use rtd_crypto::server_salt;
use rtd_crypto::temp_keys::{NEW_NONCE_LEN, SERVER_NONCE_LEN};
use rtd_util::{Clock, Random};

use crate::message_id::{MessageId, MessageIdBuf, MessageType, new_message_id};

/// Default number of inbound message identifiers retained by the replay window.
pub const REPLAY_WINDOW: usize = 100;

/// Maximum accepted age of an inbound message identifier (300 seconds).
pub const MAX_MESSAGE_AGE: Duration = Duration::from_secs(300);

/// Maximum accepted clock skew for an inbound message identifier (30 seconds).
pub const MAX_MESSAGE_SKEW: Duration = Duration::from_secs(30);

/// Default idle timeout after which an unused session expires (300 seconds).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Maximum number of unacknowledged outbound content messages retained.
///
/// Older entries are evicted first when the bound is reached, so a peer that
/// never acknowledges cannot grow the queue without bound.
pub const MAX_PENDING_CONTENT: usize = 1024;

/// Whether an outbound message is content-related for the MTProto sequence rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageClass {
    /// A content-related message: it increments the content counter and takes an
    /// odd sequence number.
    Content,
    /// A service (non-content-related) message: it leaves the content counter
    /// unchanged and takes an even sequence number.
    Service,
}

/// Identifiers allocated for one outbound message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageSeq {
    /// The allocated 64-bit message identifier.
    pub msg_id: i64,
    /// The allocated MTProto sequence number.
    pub seq_no: i32,
}

/// A placeholder layer-binding slot for a session.
///
/// The multi-layer runtime (`rtd-tlprofile`) supplies the real negotiated
/// profile; the session only carries an opaque slot so the binding can be
/// attached later without changing the session state machine. A layer of `0`
/// means the slot is still unbound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LayerSlot(u32);

impl LayerSlot {
    /// The unbound slot.
    pub const UNBOUND: Self = Self(0);

    /// Wraps a layer number.
    #[must_use]
    pub const fn new(layer: u32) -> Self {
        Self(layer)
    }

    /// Returns the bound layer number, or `0` when unbound.
    #[must_use]
    pub const fn layer(self) -> u32 {
        self.0
    }

    /// Reports whether a layer has been bound.
    #[must_use]
    pub const fn is_bound(self) -> bool {
        self.0 != 0
    }
}

/// The stable identity of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIdentity {
    /// The session identifier carried in the encrypted-message inner header.
    pub session_id: i64,
    /// The current server salt.
    pub salt: i64,
    /// The layer-binding slot (a placeholder until the layer runtime is wired).
    pub layer_slot: LayerSlot,
}

/// The lifecycle state of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// The session is active and can exchange messages.
    Active,
    /// The session expired after being idle longer than its timeout.
    Expired,
    /// The session was disconnected.
    Disconnected,
}

/// Why a session was disconnected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// The peer destroyed the session (`destroy_session`).
    Destroyed,
    /// The underlying transport closed.
    TransportClosed,
    /// The server shut the session down.
    ServerShutdown,
}

/// The outcome of validating an inbound encrypted-message header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inbound {
    /// The message is new and should be processed.
    Accepted,
    /// The message is a retransmission already seen: acknowledge it, but do not
    /// process its payload again.
    Duplicate,
}

/// An inbound message that was rejected without processing its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// The session is no longer active (expired or disconnected).
    NotActive,
    /// The header belongs to a different session.
    SessionMismatch {
        /// The session identifier expected by this session.
        expected: i64,
        /// The session identifier found in the header.
        found: i64,
    },
    /// The message identifier is not one the server accepts from a client.
    UnexpectedType(MessageType),
    /// The message identifier is older than [`MAX_MESSAGE_AGE`].
    MessageTooOld,
    /// The message identifier is further in the future than [`MAX_MESSAGE_SKEW`].
    MessageTooNew,
    /// The message identifier is below every value in the replay window.
    OutsideReplayWindow,
}

impl core::fmt::Display for SessionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotActive => f.write_str("session is not active"),
            Self::SessionMismatch { expected, found } => {
                write!(f, "session mismatch: expected {expected}, found {found}")
            }
            Self::UnexpectedType(ty) => write!(f, "unexpected message type: {ty}"),
            Self::MessageTooOld => f.write_str("message identifier is too old"),
            Self::MessageTooNew => f.write_str("message identifier is too far in the future"),
            Self::OutsideReplayWindow => {
                f.write_str("message identifier is outside the replay window")
            }
        }
    }
}

impl core::error::Error for SessionError {}

/// An outbound content message retained until the peer acknowledges it.
///
/// The body is the unencrypted message payload; the connection actor owns
/// encryption and framing, so the session only stores what it must be able to
/// hand back verbatim for a resend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundMessage {
    /// The allocated message identifier.
    pub msg_id: i64,
    /// The allocated sequence number.
    pub seq_no: i32,
    /// The unencrypted message body.
    pub body: Vec<u8>,
}

impl OutboundMessage {
    /// Creates a retained outbound message from its parts.
    #[must_use]
    pub const fn new(msg_id: i64, seq_no: i32, body: Vec<u8>) -> Self {
        Self {
            msg_id,
            seq_no,
            body,
        }
    }
}

/// A deterministic, sans-IO MTProto session state machine.
pub struct Session {
    identity: SessionIdentity,
    clock: Arc<dyn Clock + Send + Sync>,
    idle_timeout: Duration,
    last_activity: SystemTime,
    state: SessionState,
    reason: Option<DisconnectReason>,
    /// Monotonic nanosecond position for message-identifier allocation.
    nano: i64,
    /// Number of outbound content messages allocated so far.
    content_count: i32,
    /// Sliding window of recently accepted inbound identifiers.
    replay: MessageIdBuf,
    /// Outbound content messages awaiting acknowledgement (resend source).
    pending: VecDeque<OutboundMessage>,
    max_pending: usize,
    /// Inbound content message identifiers awaiting an acknowledgement.
    acks: Vec<i64>,
}

impl Session {
    /// Creates a session that adopts `session_id` as its identity.
    ///
    /// The server normally adopts the identifier carried by a peer's first
    /// encrypted message; use [`Session::new`] to generate a fresh random one.
    #[must_use]
    pub fn with_id(clock: Arc<dyn Clock + Send + Sync>, session_id: i64) -> Self {
        let now = clock.now();
        Self {
            identity: SessionIdentity {
                session_id,
                salt: 0,
                layer_slot: LayerSlot::UNBOUND,
            },
            clock,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            last_activity: now,
            state: SessionState::Active,
            reason: None,
            nano: 0,
            content_count: 0,
            replay: MessageIdBuf::new(REPLAY_WINDOW),
            pending: VecDeque::new(),
            max_pending: MAX_PENDING_CONTENT,
            acks: Vec::new(),
        }
    }

    /// Creates a session with a fresh identifier drawn from `random`.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock + Send + Sync>, random: &mut impl Random) -> Self {
        let mut bytes = [0u8; 8];
        random.fill(&mut bytes);
        Self::with_id(clock, i64::from_le_bytes(bytes))
    }

    /// Returns a snapshot of the session identity.
    #[must_use]
    pub const fn identity(&self) -> SessionIdentity {
        self.identity
    }

    /// Returns the session identifier.
    #[must_use]
    pub const fn session_id(&self) -> i64 {
        self.identity.session_id
    }

    /// Returns the current server salt.
    #[must_use]
    pub const fn salt(&self) -> i64 {
        self.identity.salt
    }

    /// Sets the current server salt.
    pub fn set_salt(&mut self, salt: i64) {
        self.identity.salt = salt;
        self.touch();
    }

    /// Derives and stores the server salt from the exchange nonces.
    ///
    /// Uses the MTProto rule implemented by [`rtd_crypto::server_salt`], so both
    /// sides of the exchange compute the same value. Returns the new salt.
    pub fn apply_exchange_salt(
        &mut self,
        new_nonce: &[u8; NEW_NONCE_LEN],
        server_nonce: &[u8; SERVER_NONCE_LEN],
    ) -> i64 {
        let salt = server_salt(new_nonce, server_nonce);
        self.set_salt(salt);
        salt
    }

    /// Returns the layer-binding slot.
    #[must_use]
    pub const fn layer_slot(&self) -> LayerSlot {
        self.identity.layer_slot
    }

    /// Sets the layer-binding slot.
    pub fn set_layer_slot(&mut self, slot: LayerSlot) {
        self.identity.layer_slot = slot;
    }

    /// Returns the last observed lifecycle state.
    ///
    /// Expiry is driven by [`Session::poll_state`]; call that to advance the
    /// state machine after the clock has moved.
    #[must_use]
    pub const fn state(&self) -> SessionState {
        self.state
    }

    /// Advances the lifecycle state by the injected clock and returns it.
    ///
    /// An active session that has been idle for at least its timeout becomes
    /// [`SessionState::Expired`]. A disconnected session stays disconnected.
    pub fn poll_state(&mut self) -> SessionState {
        if self.state == SessionState::Active && self.idle_elapsed() >= self.idle_timeout {
            self.state = SessionState::Expired;
        }
        self.state
    }

    /// Reports whether the session is currently active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self.state, SessionState::Active)
    }

    /// Records activity at the injected clock's current time.
    pub fn touch(&mut self) {
        self.last_activity = self.clock.now();
    }

    /// Returns the idle timeout after which an active session expires.
    #[must_use]
    pub const fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// Sets the idle timeout after which an active session expires.
    pub fn set_idle_timeout(&mut self, timeout: Duration) {
        self.idle_timeout = timeout;
    }

    /// Disconnects the session and releases its queued messages.
    ///
    /// The state becomes [`SessionState::Disconnected`] and the pending-ack,
    /// resend and inbound-ack queues are cleared.
    pub fn disconnect(&mut self, reason: DisconnectReason) {
        self.state = SessionState::Disconnected;
        self.reason = Some(reason);
        self.pending.clear();
        self.acks.clear();
    }

    /// Returns why the session was disconnected, if it was.
    #[must_use]
    pub const fn disconnect_reason(&self) -> Option<DisconnectReason> {
        self.reason
    }

    /// Allocates the next message identifier and sequence number.
    ///
    /// `ty` selects the wire message type (for a server, normally
    /// [`MessageType::ServerResponse`] for a reply or [`MessageType::FromServer`]
    /// for an unsolicited message); `class` selects the MTProto content rule.
    /// Identifiers are non-decreasing even when the clock does not advance.
    pub fn allocate(&mut self, ty: MessageType, class: MessageClass) -> MessageSeq {
        let msg_id = self.next_id(ty);
        let seq_no = self.next_seq(class);
        self.touch();
        MessageSeq { msg_id, seq_no }
    }

    /// Records an outbound content message until the peer acknowledges it.
    ///
    /// Older entries are evicted once [`MAX_PENDING_CONTENT`] (or the configured
    /// bound) is reached so the queue cannot grow without bound.
    pub fn record_content(&mut self, message: OutboundMessage) {
        if self.pending.len() >= self.max_pending {
            self.pending.pop_front();
        }
        self.pending.push_back(message);
    }

    /// Drops every pending outbound message whose identifier is in `ids`.
    ///
    /// This is the acknowledgement path: an acknowledged message no longer needs
    /// to be tracked for a resend. Returns how many messages were dropped.
    pub fn acknowledge(&mut self, ids: &[i64]) -> usize {
        let before = self.pending.len();
        self.pending
            .retain(|message| !ids.contains(&message.msg_id));
        before - self.pending.len()
    }

    /// Answers a resend request, returning the still-pending messages for `ids`.
    ///
    /// Messages already acknowledged (or evicted) are not returned, so an answer
    /// never resends something the peer has confirmed.
    #[must_use]
    pub fn resend(&self, ids: &[i64]) -> Vec<OutboundMessage> {
        self.pending
            .iter()
            .filter(|message| ids.contains(&message.msg_id))
            .cloned()
            .collect()
    }

    /// Iterates the pending outbound content messages, oldest first.
    pub fn pending(&self) -> impl Iterator<Item = &OutboundMessage> {
        self.pending.iter()
    }

    /// Returns how many outbound content messages await acknowledgement.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Sets the maximum number of retained outbound content messages.
    pub fn set_max_pending(&mut self, max: usize) {
        self.max_pending = max;
    }

    /// Drains the inbound message identifiers awaiting acknowledgement.
    ///
    /// Content (odd sequence) inbound messages collected by
    /// [`Session::accept_inbound`], including tolerated duplicates, are returned
    /// and removed so the caller can emit one `msgs_ack`.
    pub fn drain_acks(&mut self) -> Vec<i64> {
        core::mem::take(&mut self.acks)
    }

    /// Returns how many inbound message identifiers await acknowledgement.
    #[must_use]
    pub fn ack_len(&self) -> usize {
        self.acks.len()
    }

    /// Validates an inbound encrypted-message header.
    ///
    /// The session must be active, the header must carry this session's
    /// identifier, the identifier must be a client message inside the accepted
    /// time window and not below the replay window. A message already seen is
    /// reported as [`Inbound::Duplicate`] rather than rejected, so a
    /// retransmission is tolerated and acknowledged. Accepted content messages
    /// (and duplicates) are queued for acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the message must be rejected without
    /// processing: a non-active session, a different session identifier, an
    /// unexpected message type, an identifier outside the time window, or one
    /// below every value in the replay window.
    pub fn accept_inbound(&mut self, header: &MessageHeader) -> Result<Inbound, SessionError> {
        if self.poll_state() != SessionState::Active {
            return Err(SessionError::NotActive);
        }
        if header.session_id != self.identity.session_id {
            return Err(SessionError::SessionMismatch {
                expected: self.identity.session_id,
                found: header.session_id,
            });
        }

        let message_id = MessageId::new(header.msg_id);
        let ty = message_id.message_type();
        if ty != MessageType::FromClient {
            return Err(SessionError::UnexpectedType(ty));
        }

        let now = self.clock.now();
        let created = message_id.unix_time();
        match now.duration_since(created) {
            Ok(age) if age > MAX_MESSAGE_AGE => return Err(SessionError::MessageTooOld),
            Err(skew) if skew.duration() > MAX_MESSAGE_SKEW => {
                return Err(SessionError::MessageTooNew);
            }
            _ => {}
        }

        let needs_ack = header.seq_no & 1 == 1;

        if self.replay.contains(header.msg_id) {
            // A retransmission: acknowledge it again, but do not reprocess.
            if needs_ack {
                self.record_ack(header.msg_id);
            }
            return Ok(Inbound::Duplicate);
        }
        if !self.replay.consume(header.msg_id) {
            return Err(SessionError::OutsideReplayWindow);
        }

        if needs_ack {
            self.record_ack(header.msg_id);
        }
        self.touch();
        Ok(Inbound::Accepted)
    }

    /// Appends `msg_id` to the acknowledgement backlog if it is not already queued.
    fn record_ack(&mut self, msg_id: i64) {
        if !self.acks.contains(&msg_id) {
            self.acks.push(msg_id);
        }
    }

    /// Allocates the next monotonic message identifier for `ty`.
    fn next_id(&mut self, ty: MessageType) -> i64 {
        /// Minimum clock resolution, in nanoseconds, between generated identifiers.
        const MIN_RESOLUTION_NANOS: i64 = 10;

        let now = self.clock.unix_nanos();
        if now > self.nano {
            self.nano = now;
        } else {
            self.nano = self.nano.wrapping_add(MIN_RESOLUTION_NANOS);
        }
        new_message_id(self.nano, ty).as_i64()
    }

    /// Computes the next sequence number under the MTProto content rule.
    fn next_seq(&mut self, class: MessageClass) -> i32 {
        let base = self.content_count.wrapping_mul(2);
        match class {
            MessageClass::Content => {
                self.content_count = self.content_count.wrapping_add(1);
                base.wrapping_add(1)
            }
            MessageClass::Service => base,
        }
    }

    /// Returns how long the session has been idle at the injected clock.
    fn idle_elapsed(&self) -> Duration {
        self.clock
            .now()
            .duration_since(self.last_activity)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_IDLE_TIMEOUT, DisconnectReason, Inbound, LayerSlot, MAX_MESSAGE_SKEW, MessageClass,
        OutboundMessage, Session, SessionError, SessionState,
    };
    use crate::message_id::{MessageType, new_message_id};
    use rtd_crypto::encrypted_message::MessageHeader;
    use rtd_util::{Clock, DeterministicRandom, ManualClock, Random};
    use std::sync::Arc;
    use std::time::{Duration, UNIX_EPOCH};

    const BASE_SECS: u64 = 1_700_000_000;

    fn clock_at(secs: u64) -> ManualClock {
        ManualClock::new(UNIX_EPOCH + Duration::from_secs(secs))
    }

    fn session_at(clock: &ManualClock, id: i64) -> Session {
        Session::with_id(Arc::new(clock.clone()), id)
    }

    fn client_header(session_id: i64, msg_id: i64, seq_no: i32) -> MessageHeader {
        MessageHeader {
            salt: 0,
            session_id,
            msg_id,
            seq_no,
        }
    }

    #[test]
    fn sequence_rule_distinguishes_content_and_service() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 1);

        // A content message is 2*N+1 and advances N; a service message is 2*N
        // and leaves N unchanged.
        assert_eq!(
            session
                .allocate(MessageType::ServerResponse, MessageClass::Content)
                .seq_no,
            1
        );
        assert_eq!(
            session
                .allocate(MessageType::FromServer, MessageClass::Service)
                .seq_no,
            2
        );
        assert_eq!(
            session
                .allocate(MessageType::ServerResponse, MessageClass::Content)
                .seq_no,
            3
        );
        assert_eq!(
            session
                .allocate(MessageType::FromServer, MessageClass::Service)
                .seq_no,
            4
        );
        assert_eq!(
            session
                .allocate(MessageType::ServerResponse, MessageClass::Content)
                .seq_no,
            5
        );
    }

    #[test]
    fn allocated_identifiers_are_monotonic() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 1);
        // The clock does not advance between allocations, so the generator must
        // fall back to a minimum resolution step to keep identifiers unique.
        let first = session
            .allocate(MessageType::FromServer, MessageClass::Service)
            .msg_id;
        let second = session
            .allocate(MessageType::FromServer, MessageClass::Service)
            .msg_id;
        let third = session
            .allocate(MessageType::FromServer, MessageClass::Service)
            .msg_id;
        assert!(first < second);
        assert!(second < third);
    }

    #[test]
    fn replay_window_rejects_an_identifier_below_it() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 42);
        let nanos = (BASE_SECS as i64) * 1_000_000_000;

        // Accept a stream of distinct, increasing identifiers inside the window.
        let mut accepted = Vec::new();
        for i in 1..=200i64 {
            let msg_id = new_message_id(nanos + i * 4, MessageType::FromClient).as_i64();
            accepted.push(msg_id);
            let header = client_header(42, msg_id, 1);
            assert_eq!(session.accept_inbound(&header), Ok(Inbound::Accepted));
        }

        // A much earlier (but still time-valid) identifier is below the window.
        let old = new_message_id(nanos - 1_000_000, MessageType::FromClient).as_i64();
        assert_eq!(
            session.accept_inbound(&client_header(42, old, 1)),
            Err(SessionError::OutsideReplayWindow)
        );
    }

    #[test]
    fn duplicate_retransmission_is_tolerated() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 7);
        let nanos = (BASE_SECS as i64) * 1_000_000_000;
        let msg_id = new_message_id(nanos + 40, MessageType::FromClient).as_i64();
        let header = client_header(7, msg_id, 1);

        assert_eq!(session.accept_inbound(&header), Ok(Inbound::Accepted));
        assert_eq!(session.accept_inbound(&header), Ok(Inbound::Duplicate));
        // Both the original and the retransmission are queued for acknowledgement,
        // and the duplicate is de-duplicated in the backlog.
        assert_eq!(session.ack_len(), 1);
        assert_eq!(session.drain_acks(), vec![msg_id]);
        assert_eq!(session.ack_len(), 0);
    }

    #[test]
    fn inbound_outside_time_window_is_rejected() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 3);
        let nanos = (BASE_SECS as i64) * 1_000_000_000;

        let too_old = new_message_id(nanos - 301 * 1_000_000_000, MessageType::FromClient).as_i64();
        assert_eq!(
            session.accept_inbound(&client_header(3, too_old, 1)),
            Err(SessionError::MessageTooOld)
        );

        let future_skew = MAX_MESSAGE_SKEW + Duration::from_secs(1);
        let too_new = new_message_id(
            nanos + i64::try_from(future_skew.as_nanos()).unwrap(),
            MessageType::FromClient,
        )
        .as_i64();
        assert_eq!(
            session.accept_inbound(&client_header(3, too_new, 1)),
            Err(SessionError::MessageTooNew)
        );
    }

    #[test]
    fn foreign_session_and_type_are_rejected() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 5);
        let nanos = (BASE_SECS as i64) * 1_000_000_000;
        let msg_id = new_message_id(nanos + 40, MessageType::FromClient).as_i64();

        assert_eq!(
            session.accept_inbound(&client_header(6, msg_id, 1)),
            Err(SessionError::SessionMismatch {
                expected: 5,
                found: 6,
            })
        );

        let server_id = new_message_id(nanos + 40, MessageType::FromServer).as_i64();
        assert_eq!(
            session.accept_inbound(&client_header(5, server_id, 1)),
            Err(SessionError::UnexpectedType(MessageType::FromServer))
        );
    }

    #[test]
    fn acknowledge_drops_pending_and_resend_answers() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 11);

        session.record_content(OutboundMessage::new(100, 1, vec![1]));
        session.record_content(OutboundMessage::new(101, 3, vec![2]));
        session.record_content(OutboundMessage::new(102, 5, vec![3]));
        assert_eq!(session.pending_len(), 3);

        // A resend request is answered with exactly the still-pending messages.
        let resent = session.resend(&[101, 102, 999]);
        assert_eq!(resent.len(), 2);
        assert_eq!(resent[0].msg_id, 101);
        assert_eq!(resent[1].msg_id, 102);

        // Acknowledgement drops the confirmed message so it is not resent again.
        assert_eq!(session.acknowledge(&[101]), 1);
        assert_eq!(session.pending_len(), 2);
        assert!(session.resend(&[101]).is_empty());
        assert_eq!(session.acknowledge(&[404]), 0);
    }

    #[test]
    fn pending_queue_is_bounded() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 13);
        session.set_max_pending(2);
        session.record_content(OutboundMessage::new(1, 1, vec![1]));
        session.record_content(OutboundMessage::new(2, 3, vec![2]));
        session.record_content(OutboundMessage::new(3, 5, vec![3]));
        assert_eq!(session.pending_len(), 2);
        let ids: Vec<i64> = session.pending().map(|message| message.msg_id).collect();
        assert_eq!(ids, vec![2, 3]);
    }

    #[test]
    fn session_expires_after_idle_timeout() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 17);
        session.set_idle_timeout(Duration::from_secs(60));
        assert_eq!(session.state(), SessionState::Active);

        clock.advance(Duration::from_secs(59));
        assert_eq!(session.poll_state(), SessionState::Active);

        clock.advance(Duration::from_secs(2));
        assert_eq!(session.poll_state(), SessionState::Expired);
        assert!(!session.is_active());

        // An expired session refuses inbound messages.
        let nanos = clock.unix_nanos() + 4;
        let msg_id = new_message_id(nanos, MessageType::FromClient).as_i64();
        assert_eq!(
            session.accept_inbound(&client_header(17, msg_id, 1)),
            Err(SessionError::NotActive)
        );
    }

    #[test]
    fn activity_defers_expiry() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 19);
        session.set_idle_timeout(Duration::from_secs(60));

        clock.advance(Duration::from_secs(50));
        session.touch();
        clock.advance(Duration::from_secs(50));
        assert_eq!(session.poll_state(), SessionState::Active);
    }

    #[test]
    fn disconnect_clears_queues_and_is_sticky() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 23);
        session.record_content(OutboundMessage::new(1, 1, vec![1]));
        session.disconnect(DisconnectReason::TransportClosed);
        assert_eq!(session.state(), SessionState::Disconnected);
        assert_eq!(
            session.disconnect_reason(),
            Some(DisconnectReason::TransportClosed)
        );
        assert_eq!(session.pending_len(), 0);
        // Expiry never resurrects a disconnected session.
        clock.advance(DEFAULT_IDLE_TIMEOUT + Duration::from_secs(1));
        assert_eq!(session.poll_state(), SessionState::Disconnected);
    }

    #[test]
    fn identity_carries_salt_and_layer_slot() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 29);
        assert_eq!(session.layer_slot(), LayerSlot::UNBOUND);
        session.set_layer_slot(LayerSlot::new(232));
        session.set_salt(0x1122_3344_5566_7788);

        let identity = session.identity();
        assert_eq!(identity.session_id, 29);
        assert_eq!(identity.salt, 0x1122_3344_5566_7788);
        assert_eq!(identity.layer_slot, LayerSlot::new(232));
        assert!(identity.layer_slot.is_bound());
        assert_eq!(session.salt(), 0x1122_3344_5566_7788);
        assert_eq!(session.layer_slot().layer(), 232);
    }

    #[test]
    fn exchange_salt_matches_the_crypto_rule() {
        let clock = clock_at(BASE_SECS);
        let mut session = session_at(&clock, 31);
        let new_nonce = [0x11u8; 32];
        let server_nonce = [0x22u8; 16];
        let salt = session.apply_exchange_salt(&new_nonce, &server_nonce);
        assert_eq!(salt, rtd_crypto::server_salt(&new_nonce, &server_nonce));
        assert_eq!(session.salt(), salt);
    }

    #[test]
    fn new_session_draws_its_identifier_from_the_injected_random() {
        let clock = clock_at(BASE_SECS);
        let mut random = DeterministicRandom::new(7);
        let session = Session::new(Arc::new(clock), &mut random);
        let mut expected = [0u8; 8];
        let mut repeat = DeterministicRandom::new(7);
        repeat.fill(&mut expected);
        assert_eq!(session.session_id(), i64::from_le_bytes(expected));
    }
}
