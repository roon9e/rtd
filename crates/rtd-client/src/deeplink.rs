//! Telegram deep-link parsing.
//!
//! Supports the `tg://` scheme and the `t.me`/`telegram.me`/`telegram.dog` web
//! forms, yielding a typed [`Deeplink`].

use std::collections::HashMap;

/// A parsed deep link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deeplink {
    /// A link to a username, optionally with a start payload.
    Resolve {
        /// The target username.
        username: String,
        /// The optional `start` payload.
        start: Option<String>,
    },
    /// An invite link with its hash.
    Join {
        /// The invite hash.
        hash: String,
    },
}

/// A deep-link parse failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeeplinkError {
    /// The string is not a Telegram deep link.
    NotADeeplink,
    /// The link is well-formed but missing a required parameter.
    Invalid(&'static str),
}

/// The web hosts recognised as Telegram deep links.
const WEB_HOSTS: [&str; 3] = ["t.me", "telegram.me", "telegram.dog"];

/// Parses a Telegram deep link.
///
/// # Errors
///
/// Returns [`DeeplinkError::NotADeeplink`] when the string is not a deep link and
/// [`DeeplinkError::Invalid`] when it is missing a required part.
pub fn parse(url: &str) -> Result<Deeplink, DeeplinkError> {
    let url = url.trim();
    if let Some(rest) = url
        .strip_prefix("tg://")
        .or_else(|| url.strip_prefix("tg:"))
    {
        return parse_tg(rest);
    }
    for prefix in ["https://", "http://"] {
        if let Some(rest) = url.strip_prefix(prefix) {
            return parse_web(rest);
        }
    }
    Err(DeeplinkError::NotADeeplink)
}

/// Reports whether `url` is a recognised Telegram deep link.
#[must_use]
pub fn is_deeplink_like(url: &str) -> bool {
    parse(url).is_ok()
}

/// Parses a `tg:` link body (for example `resolve?domain=durov`).
fn parse_tg(rest: &str) -> Result<Deeplink, DeeplinkError> {
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (rest, None),
    };
    let path = path.trim_start_matches('/');
    let parameters = query.map(parse_query).unwrap_or_default();
    match path {
        "resolve" => {
            let username = parameters
                .get("domain")
                .ok_or(DeeplinkError::Invalid("resolve without domain"))?;
            Ok(Deeplink::Resolve {
                username: username.clone(),
                start: parameters.get("start").cloned(),
            })
        }
        "join" => {
            let hash = parameters
                .get("invite")
                .ok_or(DeeplinkError::Invalid("join without invite"))?;
            Ok(Deeplink::Join { hash: hash.clone() })
        }
        _ => Err(DeeplinkError::NotADeeplink),
    }
}

/// Parses a web link body (for example `t.me/durov`).
fn parse_web(rest: &str) -> Result<Deeplink, DeeplinkError> {
    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host, path),
        None => (rest, ""),
    };
    let host = host.split(':').next().unwrap_or(host);
    if !WEB_HOSTS
        .iter()
        .any(|known| host.eq_ignore_ascii_case(known))
    {
        return Err(DeeplinkError::NotADeeplink);
    }
    if let Some(hash) = path.strip_prefix("joinchat/") {
        return Ok(Deeplink::Join {
            hash: hash.to_owned(),
        });
    }
    if let Some(hash) = path.strip_prefix('+') {
        return Ok(Deeplink::Join {
            hash: hash.to_owned(),
        });
    }
    let username = path.trim_end_matches('/');
    if username.is_empty() || username.starts_with("s/") || username.starts_with("c/") {
        return Err(DeeplinkError::NotADeeplink);
    }
    Ok(Deeplink::Resolve {
        username: username.to_owned(),
        start: None,
    })
}

/// Parses a query string into a parameter map (last value wins).
fn parse_query(query: &str) -> HashMap<String, String> {
    let mut parameters = HashMap::new();
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            parameters.insert(key.to_owned(), value.to_owned());
        }
    }
    parameters
}

#[cfg(test)]
mod tests {
    use super::{Deeplink, DeeplinkError, is_deeplink_like, parse};

    #[test]
    fn parses_tg_resolve() {
        assert_eq!(
            parse("tg://resolve?domain=durov&start=payload").unwrap(),
            Deeplink::Resolve {
                username: "durov".to_owned(),
                start: Some("payload".to_owned()),
            }
        );
    }

    #[test]
    fn parses_tg_join() {
        assert_eq!(
            parse("tg://join?invite=AbCdEf").unwrap(),
            Deeplink::Join {
                hash: "AbCdEf".to_owned(),
            }
        );
    }

    #[test]
    fn parses_web_links() {
        assert_eq!(
            parse("https://t.me/durov").unwrap(),
            Deeplink::Resolve {
                username: "durov".to_owned(),
                start: None,
            }
        );
        assert_eq!(
            parse("https://t.me/joinchat/AbCdEf").unwrap(),
            Deeplink::Join {
                hash: "AbCdEf".to_owned(),
            }
        );
        assert_eq!(
            parse("https://t.me/+AbCdEf").unwrap(),
            Deeplink::Join {
                hash: "AbCdEf".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_non_deeplinks() {
        assert_eq!(
            parse("https://example.com/x"),
            Err(DeeplinkError::NotADeeplink)
        );
        assert!(!is_deeplink_like("hello"));
        assert_eq!(
            parse("tg://resolve"),
            Err(DeeplinkError::Invalid("resolve without domain"))
        );
    }
}
