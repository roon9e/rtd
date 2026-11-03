//! QR login token helpers.

use crate::error::Result;

/// An exported login token with the URL a client renders as a QR code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginToken {
    /// The raw token bytes.
    pub token: Vec<u8>,
    /// The `tg://login?token=...` URL.
    pub url: String,
    /// The Unix expiry time in seconds.
    pub expires: i32,
}

impl LoginToken {
    /// Builds a token view from the raw `expires` and `token`.
    #[must_use]
    pub fn new(expires: i32, token: Vec<u8>) -> Self {
        let url = login_url(&token);
        Self {
            token,
            url,
            expires,
        }
    }
}

/// Returns the `tg://login?token=...` URL for a raw token (base64url, no padding).
#[must_use]
pub fn login_url(token: &[u8]) -> String {
    format!("tg://login?token={}", base64_url(token))
}

/// Drives the QR login loop: export a token, hand it to `on_token`, then import
/// it until the server authorizes the session.
///
/// `import` returns `Ok(None)` while the token has not yet been scanned and
/// confirmed, and `Ok(Some(value))` once it has. On each iteration a fresh token
/// is exported (the previous one may have expired).
///
/// # Errors
///
/// Returns the first error reported by `export` or `import`.
pub fn drive_login<T>(
    mut export: impl FnMut() -> Result<LoginToken>,
    mut import: impl FnMut(&[u8]) -> Result<Option<T>>,
    mut on_token: impl FnMut(&LoginToken),
) -> Result<T> {
    loop {
        let token = export()?;
        on_token(&token);
        if let Some(value) = import(&token.token)? {
            return Ok(value);
        }
    }
}

/// Encodes `data` as unpadded URL-safe base64.
fn base64_url(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from);
        let b2 = chunk.get(2).copied().map(u32::from);
        let triple = (b0 << 16) | (b1.unwrap_or(0) << 8) | b2.unwrap_or(0);
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        if b1.is_some() {
            out.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        }
        if b2.is_some() {
            out.push(ALPHABET[(triple & 0x3f) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{LoginToken, login_url};

    #[test]
    fn url_is_unpadded_base64url() {
        // 0xfb 0xff encodes to "-_8" in URL-safe base64 (no padding).
        assert_eq!(login_url(&[0xfb, 0xff]), "tg://login?token=-_8");
        let token = LoginToken::new(42, b"abc".to_vec());
        assert_eq!(token.url, "tg://login?token=YWJj");
        assert_eq!(token.expires, 42);
    }

    #[test]
    fn drive_login_loops_until_authorized() {
        let mut exports = vec![
            LoginToken::new(1, b"one".to_vec()),
            LoginToken::new(2, b"two".to_vec()),
            LoginToken::new(3, b"three".to_vec()),
        ]
        .into_iter();
        let mut seen = Vec::new();
        let mut imports = 0;
        let result = super::drive_login(
            || Ok(exports.next().expect("token")),
            |token| {
                imports += 1;
                if imports >= 2 {
                    Ok(Some(token.len()))
                } else {
                    Ok(None)
                }
            },
            |token| seen.push(token.url.clone()),
        )
        .unwrap();
        assert_eq!(result, 3);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], "tg://login?token=b25l");
    }
}
