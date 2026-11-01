//! Chat-invite webview query-id hooks.
//!
//! In layer 227 a `messages.chatInviteJoinResultWebView` carried the join query
//! id inside its nested `webview` result rather than as its own field. The
//! canonical shape has a dedicated `query_id`, so decoding must capture it from
//! the webview and then require it.

use rtd_tg::WebViewResult;

use crate::error::{HookError, Result};

/// Profile to canonical: capture the join query id from the nested `webview`.
///
/// # Errors
///
/// Returns [`HookError::Missing`] when the webview omits its query id.
pub fn capture_query_id_from_webview(webview: &WebViewResult) -> Result<i64> {
    match webview {
        WebViewResult::WebViewResultUrl {
            query_id: Some(query_id),
            ..
        } => Ok(*query_id),
        WebViewResult::WebViewResultUrl { query_id: None, .. } => {
            Err(HookError::Missing("chat invite webview query_id"))
        }
    }
}

/// Profile to canonical: require the captured query id to be present.
///
/// # Errors
///
/// Returns [`HookError::Missing`] when the query id was not captured.
pub fn require_captured_query_id(query_id: Option<i64>) -> Result<i64> {
    query_id.ok_or(HookError::Missing("chat invite query_id"))
}

#[cfg(test)]
mod tests {
    use rtd_tg::WebViewResult;

    use super::{capture_query_id_from_webview, require_captured_query_id};
    use crate::error::HookError;

    fn webview(query_id: Option<i64>) -> WebViewResult {
        WebViewResult::WebViewResultUrl {
            flags: 0,
            fullsize: false,
            fullscreen: false,
            same_origin: false,
            query_id,
            url: "https://example.test".to_owned(),
        }
    }

    #[test]
    fn captures_the_webview_query_id() {
        assert_eq!(
            capture_query_id_from_webview(&webview(Some(42))).unwrap(),
            42
        );
        assert_eq!(require_captured_query_id(Some(42)).unwrap(), 42);
    }

    #[test]
    fn a_missing_query_id_fails_closed() {
        assert!(matches!(
            capture_query_id_from_webview(&webview(None)).unwrap_err(),
            HookError::Missing(_)
        ));
        assert!(matches!(
            require_captured_query_id(None).unwrap_err(),
            HookError::Missing(_)
        ));
    }
}
