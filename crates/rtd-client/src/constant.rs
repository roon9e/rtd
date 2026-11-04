//! Telegram-defined constants and TDLib peer-id encoding.
//!
//! Mirrors `td/constant`: well-known user IDs, upload limits, test
//! credentials, and [`TdlibPeerId`], the TDLib-style peer identifier that packs
//! users, chats and channels into one `i64` namespace.

/// Service notification user ID.
pub const SERVICE_NOTIFICATION_USER_ID: i64 = 777_000;
/// Special user ID of the "Replies" bot.
pub const REPLIES_BOT_USER_ID: i64 = 1_271_266_957;
/// [`REPLIES_BOT_USER_ID`] on test DCs.
pub const TEST_REPLIES_BOT_USER_ID: i64 = 708_513;
/// User ID of the bot shown as the sender of anonymous group messages on old clients.
pub const ANONYMOUS_BOT_USER_ID: i64 = 1_087_968_824;
/// [`ANONYMOUS_BOT_USER_ID`] on test DCs.
pub const TEST_ANONYMOUS_BOT_USER_ID: i64 = 552_888;
/// User ID of the "Channel Bot", the sender of anonymous channel messages.
pub const CHANNEL_BOT_USER_ID: i64 = 136_817_688;
/// [`CHANNEL_BOT_USER_ID`] on test DCs.
pub const TEST_CHANNEL_BOT_USER_ID: i64 = 936_174;

/// Test-only application id accepted by the test DCs.
pub const TEST_APP_ID: i32 = 17_349;
/// Test-only application hash matching [`TEST_APP_ID`].
pub const TEST_APP_HASH: &str = "344583e45741c457fe1862106095a5eb";

/// Maximum size, in bytes, of a file uploaded through `upload.saveFilePart`.
///
/// Larger files use `upload.saveBigFilePart`.
pub const UPLOAD_MAX_SMALL_SIZE: usize = 10 * 1024 * 1024;
/// Maximum number of parts in a big upload (part indices `0..=3999`).
pub const UPLOAD_MAX_PARTS: usize = 3_999;
/// Required alignment of an upload part size, in bytes.
pub const UPLOAD_PADDING: usize = 1_024;
/// Maximum size, in bytes, of a single upload part.
pub const UPLOAD_MAX_PART_SIZE: usize = 524_288;

/// Maximum TDLib chat ID.
pub const MAX_TDLIB_CHAT_ID: i64 = 999_999_999_999;
/// Maximum TDLib channel ID (non-monoforum).
pub const MAX_TDLIB_CHANNEL_ID: i64 = 1_000_000_000_000 - (1_i64 << 31);
/// Base value for TDLib channel IDs.
pub const ZERO_TDLIB_CHANNEL_ID: i64 = -1_000_000_000_000;
/// Minimum TDLib secret-chat ID.
pub const ZERO_TDLIB_SECRET_CHAT_ID: i64 = -2_000_000_000_000;
/// Maximum TDLib user ID.
pub const MAX_TDLIB_USER_ID: i64 = (1_i64 << 40) - 1;
/// Minimum MTProto ID for monoforums.
pub const MIN_MTPROTO_MONOFORUM_ID: i64 = 1_002_147_483_649;
/// Maximum MTProto ID for monoforums.
pub const MAX_MTPROTO_MONOFORUM_ID: i64 = 3_000_000_000_000;
/// Minimum Bot-API ID for monoforums.
pub const MIN_BOTAPI_MONOFORUM_ID: i64 = -4_000_000_000_000;
/// Maximum Bot-API ID for monoforums.
pub const MAX_BOTAPI_MONOFORUM_ID: i64 = -2_002_147_483_649;

/// A TDLib-style peer identifier.
///
/// Users are positive, chats are negated, and channels are offset from
/// [`ZERO_TDLIB_CHANNEL_ID`]. Use [`Self::to_plain`] to recover the MTProto ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TdlibPeerId(i64);

impl TdlibPeerId {
    /// Wraps a raw TDLib peer id.
    #[must_use]
    pub const fn new(raw: i64) -> Self {
        Self(raw)
    }

    /// Builds a user peer id from a plain (MTProto) user id.
    #[must_use]
    pub const fn user(plain: i64) -> Self {
        Self(plain)
    }

    /// Builds a chat peer id from a plain (MTProto) chat id.
    #[must_use]
    pub const fn chat(plain: i64) -> Self {
        Self(-plain)
    }

    /// Builds a channel peer id from a plain (MTProto) channel id.
    #[must_use]
    pub const fn channel(plain: i64) -> Self {
        Self(ZERO_TDLIB_CHANNEL_ID - plain)
    }

    /// Returns the raw TDLib id.
    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }

    /// Converts to the plain (MTProto) id.
    ///
    /// Returns `0` for an id that is neither a user, chat nor channel.
    #[must_use]
    pub const fn to_plain(self) -> i64 {
        if self.is_user() {
            self.0
        } else if self.is_chat() {
            -self.0
        } else if self.is_channel() {
            -(self.0 - ZERO_TDLIB_CHANNEL_ID)
        } else {
            0
        }
    }

    /// Returns whether this id is a user.
    #[must_use]
    pub const fn is_user(self) -> bool {
        self.0 > 0 && self.0 <= MAX_TDLIB_USER_ID
    }

    /// Returns whether this id is a (small) chat.
    #[must_use]
    pub const fn is_chat(self) -> bool {
        self.0 < 0 && self.0 >= -MAX_TDLIB_CHAT_ID
    }

    /// Returns whether this id is a monoforum.
    #[must_use]
    pub const fn is_monoforum(self) -> bool {
        self.0 >= MIN_BOTAPI_MONOFORUM_ID && self.0 <= MAX_BOTAPI_MONOFORUM_ID
    }

    /// Returns whether this id is a channel (including monoforums).
    #[must_use]
    pub const fn is_channel(self) -> bool {
        if self.0 >= 0 || self.0 == ZERO_TDLIB_CHANNEL_ID || self.is_chat() {
            return false;
        }
        // Regular channels.
        if self.0 >= -1_997_852_516_352 && self.0 <= -1_000_000_000_001 {
            return true;
        }
        // Monoforums.
        self.is_monoforum()
    }
}

#[cfg(test)]
mod tests {
    use super::TdlibPeerId;

    #[test]
    fn classifies_ids() {
        assert!(TdlibPeerId::new(309_570_373).is_user());
        assert!(TdlibPeerId::new(140_267_078).is_user());
        assert!(TdlibPeerId::new(-365_219_918).is_chat());
        assert!(TdlibPeerId::new(-1_001_228_418_968).is_channel());
    }

    #[test]
    fn round_trips_plain_ids() {
        let cases = [
            TdlibPeerId::new(309_570_373),
            TdlibPeerId::new(140_267_078),
            TdlibPeerId::new(-365_219_918),
            TdlibPeerId::new(-1_001_228_418_968),
        ];
        for id in cases {
            let plain = id.to_plain();
            let rebuilt = if id.is_user() {
                TdlibPeerId::user(plain)
            } else if id.is_chat() {
                TdlibPeerId::chat(plain)
            } else {
                TdlibPeerId::channel(plain)
            };
            assert_eq!(id, rebuilt);
        }
        assert_eq!(
            TdlibPeerId::new(-1_001_228_418_968).to_plain(),
            1_228_418_968
        );
    }
}
