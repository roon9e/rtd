//! Telegram application credentials.
//!
//! The Go tools default to the public open-source TDesktop application
//! id/hash and let `stickerfetch`/`giftfetch` override them through
//! `TG_API_ID`/`TG_API_HASH` (the published id is rate-limited with
//! `API_ID_PUBLISHED_FLOOD`). This mirrors that behaviour.

/// The public open-source TDesktop application id.
pub const TDESKTOP_API_ID: i32 = 17349;

/// The public open-source TDesktop application hash.
pub const TDESKTOP_API_HASH: &str = "344583e45741c457fe1862106095a5eb";

/// Returns the application credentials, preferring `TG_API_ID`/`TG_API_HASH`.
///
/// `default_id`/`default_hash` are used when the environment does not supply a
/// valid override.
#[must_use]
pub fn api_credentials(default_id: i32, default_hash: &str) -> (i32, String) {
    let mut id = default_id;
    let mut hash = default_hash.to_owned();
    if let Ok(value) = std::env::var("TG_API_ID")
        && let Ok(parsed) = value.trim().parse::<i32>()
    {
        id = parsed;
    }
    if let Ok(value) = std::env::var("TG_API_HASH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            hash = trimmed.to_owned();
        }
    }
    (id, hash)
}

/// Returns the session path from `SESSION`, or `default` when unset.
#[must_use]
pub fn session_path(default: &str) -> String {
    match std::env::var("SESSION") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => default.to_owned(),
    }
}

/// Returns the value of an environment variable, treating an empty value as unset.
#[must_use]
pub fn env_nonempty(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}
