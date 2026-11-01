//! MTProto message identifiers and replay mitigation.
//!
//! Message identifiers are coupled to message creation time: the high 32 bits
//! hold the Unix timestamp in seconds while the low 32 bits hold a fraction of
//! the second with the low two bits replaced by a message-type yield.
//!
//! See <https://core.telegram.org/mtproto/description#message-identifier-msg-id>.

use core::fmt;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The low-bit modulo used to encode the message type.
const MESSAGE_ID_MODULO: i64 = 4;

/// Encoded yield for client messages.
const YIELD_CLIENT: i64 = 0;
/// Encoded yield for server responses.
const YIELD_SERVER_RESPONSE: i64 = 1;
/// Encoded yield for unsolicited server messages.
const YIELD_FROM_SERVER: i64 = 3;

/// Minimum clock resolution, in nanoseconds, between generated identifiers.
const MIN_RESOLUTION_NANOS: i64 = 10;

/// Number of nanoseconds in one second.
const NANOS_PER_SECOND: i64 = 1_000_000_000;

/// Type of a message, derived from the low bits of its identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MessageType {
    /// The identifier has an unknown type and should probably be ignored.
    Unknown,
    /// A message sent by the client.
    FromClient,
    /// A response to a client message.
    ServerResponse,
    /// An unsolicited message from the server.
    FromServer,
}

impl MessageType {
    /// Returns the yield value that encodes this type in an identifier.
    const fn yield_value(self) -> i64 {
        match self {
            Self::FromClient | Self::Unknown => YIELD_CLIENT,
            Self::ServerResponse => YIELD_SERVER_RESPONSE,
            Self::FromServer => YIELD_FROM_SERVER,
        }
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Unknown => "Unknown",
            Self::FromClient => "FromClient",
            Self::ServerResponse => "ServerResponse",
            Self::FromServer => "FromServer",
        };
        f.write_str(name)
    }
}

/// A 64-bit MTProto message identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MessageId(i64);

impl MessageId {
    /// Wraps a raw identifier.
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    /// Returns the raw identifier.
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self.0
    }

    /// Consumes the value and returns the raw identifier.
    #[must_use]
    pub const fn into_i64(self) -> i64 {
        self.0
    }

    /// Returns the approximate time at which the identifier was generated.
    #[must_use]
    pub fn unix_time(self) -> SystemTime {
        let int_part = self.0 >> 32;
        // Sign-extend the low 32 bits, matching Go's `int32(id)` conversion.
        let frac_part = i64::from(self.0 as i32);
        let total_nanos = int_part
            .wrapping_mul(NANOS_PER_SECOND)
            .wrapping_add(frac_part);
        if total_nanos >= 0 {
            UNIX_EPOCH + Duration::from_nanos(total_nanos as u64)
        } else {
            UNIX_EPOCH - Duration::from_nanos(total_nanos.unsigned_abs())
        }
    }

    /// Returns the message type derived from the identifier's low bits.
    #[must_use]
    pub fn message_type(self) -> MessageType {
        match self.0 % MESSAGE_ID_MODULO {
            YIELD_CLIENT => MessageType::FromClient,
            YIELD_SERVER_RESPONSE => MessageType::ServerResponse,
            YIELD_FROM_SERVER => MessageType::FromServer,
            _ => MessageType::Unknown,
        }
    }
}

impl From<i64> for MessageId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<MessageId> for i64 {
    fn from(id: MessageId) -> Self {
        id.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:x} ({}, {:?})",
            self.0,
            self.message_type(),
            self.unix_time()
        )
    }
}

/// Converts a [`SystemTime`] into a Unix timestamp in nanoseconds.
fn system_time_to_nanos(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i64,
        Err(err) => -(err.duration().as_nanos() as i64),
    }
}

/// Builds a message identifier for the given nanosecond timestamp and type.
#[must_use]
pub fn new_message_id(nanos: i64, ty: MessageType) -> MessageId {
    let int_part = nanos / NANOS_PER_SECOND;
    let mut frac_part = nanos % NANOS_PER_SECOND;

    // Ensure that `frac_part % 4 == 0`, then add the modulo 4 yield.
    frac_part &= !(MESSAGE_ID_MODULO - 1);
    frac_part = frac_part.wrapping_add(ty.yield_value());

    let mut id = MessageId((int_part << 32) | frac_part);
    // A client message created exactly on an integral second would otherwise
    // have a zero lower 32-bit fractional part, which MTProto reserves to make
    // replay protection effective. Move it to the first valid client tick while
    // preserving both its timestamp and message type.
    if id.message_type() == MessageType::FromClient && (id.0 as u32) == 0 {
        id.0 = id.0.wrapping_add(MESSAGE_ID_MODULO);
    }
    id
}

/// Builds a message identifier for the given time and type.
#[must_use]
pub fn new_message_id_time(time: SystemTime, ty: MessageType) -> MessageId {
    new_message_id(system_time_to_nanos(time), ty)
}

/// State guarded by a [`MessageIdGen`]'s mutex.
struct GenState {
    nano: i64,
    now: Box<dyn Fn() -> SystemTime + Send + Sync>,
}

/// A message identifier generator that prevents collisions caused by low
/// system clock resolution.
pub struct MessageIdGen {
    state: Mutex<GenState>,
}

impl MessageIdGen {
    /// Creates a generator backed by the system clock.
    #[must_use]
    pub fn system() -> Self {
        Self::with_now(SystemTime::now)
    }

    /// Creates a generator whose current time is provided by `now`.
    ///
    /// The closure is called under the generator's mutex, so it does not need
    /// to be reentrant.
    pub fn with_now<F>(now: F) -> Self
    where
        F: Fn() -> SystemTime + Send + Sync + 'static,
    {
        Self {
            state: Mutex::new(GenState {
                nano: 0,
                now: Box::new(now),
            }),
        }
    }

    /// Generates the next identifier for the given type.
    ///
    /// Identifiers are monotonically non-decreasing even when the clock does
    /// not advance between calls: in that case the previous timestamp is
    /// advanced by [`MIN_RESOLUTION_NANOS`].
    pub fn generate(&self, ty: MessageType) -> MessageId {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let nano = system_time_to_nanos((state.now)());
        if nano > state.nano {
            state.nano = nano;
        } else {
            state.nano = state.nano.wrapping_add(MIN_RESOLUTION_NANOS);
        }
        new_message_id(state.nano, ty)
    }
}

impl Default for MessageIdGen {
    fn default() -> Self {
        Self::system()
    }
}

/// A fixed-size sliding window of recently seen message identifiers.
///
/// This replicates the MTProto security-guideline check: a message is rejected
/// if its identifier equals a stored value or is lower than all stored values;
/// otherwise the lowest stored value is replaced.
pub struct MessageIdBuf {
    buf: Vec<i64>,
}

impl MessageIdBuf {
    /// Creates a window that stores the last `n` identifiers.
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self { buf: vec![0; n] }
    }

    /// Consumes `new_id`, returning `false` if the message must be discarded.
    pub fn consume(&mut self, new_id: i64) -> bool {
        // The minimum must start above every possible identifier so that the
        // scan finds the true lowest stored value even after the initial
        // zero-filled slots have all been replaced.
        let mut min_idx = 0;
        let mut min_id = i64::MAX;
        for (i, &id) in self.buf.iter().enumerate() {
            if id == new_id {
                // Equal to a stored value.
                return false;
            }
            if id < min_id {
                min_idx = i;
                min_id = id;
            }
        }
        if new_id < min_id {
            // Lower than all stored values.
            return false;
        }

        // Message is accepted: replace the lowest stored identifier.
        self.buf[min_idx] = new_id;
        true
    }

    /// Reports whether `id` is currently stored in the window.
    ///
    /// This distinguishes a duplicate retransmission (the identifier is already
    /// present and should be tolerated) from an identifier that is merely lower
    /// than every stored value and therefore falls outside the window.
    #[must_use]
    pub fn contains(&self, id: i64) -> bool {
        self.buf.contains(&id)
    }

    /// Returns the configured window size.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns `true` when the window stores no identifiers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MessageId, MessageIdBuf, MessageIdGen, MessageType, new_message_id, new_message_id_time,
    };
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn golden_id_and_type() {
        // 2018-10-10T23:42:06.000013600Z, matching the reference test vector.
        let now = UNIX_EPOCH + Duration::new(0x5bbe_8e4e, 13_600);
        let id = new_message_id_time(now, MessageType::FromClient);
        assert_eq!(id.as_i64(), 0x5bbe_8e4e_0000_3520);
        assert_eq!(id.message_type(), MessageType::FromClient);
        assert_eq!(id.unix_time(), now);
    }

    #[test]
    fn type_yields() {
        let now = UNIX_EPOCH + Duration::new(1_000, 0);
        assert_eq!(
            new_message_id_time(now, MessageType::ServerResponse).message_type(),
            MessageType::ServerResponse
        );
        assert_eq!(
            new_message_id_time(now, MessageType::FromServer).message_type(),
            MessageType::FromServer
        );
        // Unknown yields like a client message.
        assert_eq!(
            new_message_id_time(now, MessageType::Unknown).message_type(),
            MessageType::FromClient
        );
    }

    #[test]
    fn integral_second_client_id_is_adjusted() {
        let exact = UNIX_EPOCH + Duration::from_secs(1_788_283_940);
        let id = new_message_id_time(exact, MessageType::FromClient);
        assert_eq!(id.message_type(), MessageType::FromClient);
        // The reserved zero fractional part is moved to the first valid client
        // tick without changing the timestamp.
        assert_eq!(id.as_i64() as u32, 4);
        assert!(id.unix_time() >= exact);
    }

    #[test]
    fn time_round_trip() {
        // For a client message the yield is zero, so any non-zero multiple of
        // four nanoseconds round-trips exactly.
        for secs in [1_u64, 1_000, 1_788_283_940] {
            for nanos in [4_u32, 8, 1_000, 999_999_996] {
                let time = UNIX_EPOCH + Duration::new(secs, nanos);
                let id = new_message_id_time(time, MessageType::FromClient);
                assert_eq!(id.unix_time(), time);
            }
        }

        // A server message has yield three, so fractional parts congruent to
        // three round-trip exactly.
        for secs in [0_u64, 1, 1_000, 1_788_283_940] {
            for nanos in [3_u32, 7, 1_003, 999_999_999] {
                let time = UNIX_EPOCH + Duration::new(secs, nanos);
                let id = new_message_id_time(time, MessageType::FromServer);
                assert_eq!(id.message_type(), MessageType::FromServer);
                assert_eq!(id.unix_time(), time);
            }
        }
    }

    #[test]
    fn raw_low_bits_are_cleared() {
        let id = new_message_id(1_000_000_007, MessageType::FromClient);
        assert_eq!(id.as_i64() & 3, 0);
    }

    #[test]
    fn generator_avoids_collisions() {
        let fixed = UNIX_EPOCH + Duration::new(1_788_283_940, 0);
        let generator = MessageIdGen::with_now(move || fixed);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1_000 {
            let id = generator.generate(MessageType::FromClient);
            assert!(seen.insert(id.as_i64()), "duplicate id generated");
        }
    }

    #[test]
    fn generator_is_monotonic() {
        let fixed = UNIX_EPOCH + Duration::new(1_788_283_940, 0);
        let generator = MessageIdGen::with_now(move || fixed);
        let first = generator.generate(MessageType::FromClient);
        let second = generator.generate(MessageType::FromClient);
        assert!(second > first);
        assert_ne!(first.as_i64() as u32, 0);
        assert_ne!(second.as_i64() as u32, 0);
    }

    #[test]
    fn replay_window_rejects_duplicates_and_outdated() {
        let mut buf = MessageIdBuf::new(10);
        // The zero-initialized window already contains zero.
        assert!(!buf.consume(0));
        assert!(buf.consume(1));
        assert!(!buf.consume(1));
        for i in 2..=20 {
            assert!(buf.consume(i), "id {i} should be accepted");
        }
        assert!(!buf.consume(-1));
        assert_eq!(buf.len(), 10);
        assert!(!buf.is_empty());
    }

    #[test]
    fn replay_window_reports_membership() {
        let mut buf = MessageIdBuf::new(4);
        // The zero-initialized window already stores zero.
        assert!(buf.contains(0));
        assert!(!buf.contains(7));
        assert!(buf.consume(7));
        assert!(buf.contains(7));
        assert!(!buf.consume(7));
        assert!(buf.contains(7));
    }

    #[test]
    fn replay_window_rejects_positive_identifier_below_it() {
        let mut buf = MessageIdBuf::new(4);
        for id in 10..=20 {
            assert!(buf.consume(id), "id {id} should be accepted");
        }
        // The window now holds the four highest identifiers, so a positive
        // identifier lower than all of them is rejected rather than slid in.
        assert!(!buf.consume(9));
        assert!(buf.consume(21));
    }

    #[test]
    fn replay_window_advances() {
        let mut buf = MessageIdBuf::new(4);
        assert!(buf.consume(5));
        assert!(buf.consume(6));
        assert!(!buf.consume(6));
        assert!(buf.consume(7));
    }

    #[test]
    fn default_generator_is_system() {
        let generator = MessageIdGen::default();
        let a = generator.generate(MessageType::FromClient);
        let b = generator.generate(MessageType::FromClient);
        assert!(b > a);
    }

    #[test]
    fn message_id_from_round_trip() {
        let id: MessageId = 42_i64.into();
        let raw: i64 = id.into();
        assert_eq!(raw, 42);
        assert_eq!(id.as_i64(), 42);
        assert_eq!(MessageId::new(42).into_i64(), 42);
    }
}
