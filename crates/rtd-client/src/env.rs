//! Environment-based configuration helpers.
//!
//! Mirrors `td`'s environment conveniences for application id/hash without
//! imposing a client constructor: callers read the values here and pass them to
//! [`crate::Client::send_code`] or [`crate::auth::Flow`].

/// Telegram application credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiCredentials {
    /// The application id.
    pub api_id: i32,
    /// The application hash.
    pub api_hash: String,
}

/// Parses credentials from an id and hash, if both are present and valid.
#[must_use]
pub fn parse_api_credentials(
    api_id: Option<&str>,
    api_hash: Option<&str>,
) -> Option<ApiCredentials> {
    let api_id = api_id?.trim().parse::<i32>().ok()?;
    let api_hash = api_hash?.trim();
    if api_hash.is_empty() {
        return None;
    }
    Some(ApiCredentials {
        api_id,
        api_hash: api_hash.to_owned(),
    })
}

/// Reads application credentials from the environment.
///
/// Checks `TELEGRAM_API_ID`/`TELEGRAM_API_HASH`, then `API_ID`/`API_HASH`.
/// Returns `None` when neither pair is complete and valid.
#[must_use]
pub fn api_credentials_from_env() -> Option<ApiCredentials> {
    for (id_key, hash_key) in [
        ("TELEGRAM_API_ID", "TELEGRAM_API_HASH"),
        ("API_ID", "API_HASH"),
    ] {
        let id = std::env::var(id_key).ok();
        let hash = std::env::var(hash_key).ok();
        if let Some(credentials) = parse_api_credentials(id.as_deref(), hash.as_deref()) {
            return Some(credentials);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{ApiCredentials, parse_api_credentials};

    #[test]
    fn parses_valid_credentials() {
        assert_eq!(
            parse_api_credentials(Some("17349"), Some(" hash ")),
            Some(ApiCredentials {
                api_id: 17349,
                api_hash: "hash".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_incomplete_credentials() {
        assert!(parse_api_credentials(None, Some("hash")).is_none());
        assert!(parse_api_credentials(Some("nope"), Some("hash")).is_none());
        assert!(parse_api_credentials(Some("1"), Some("  ")).is_none());
    }
}
