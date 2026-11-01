//! Result-level conversion hooks for older layers whose RPC result type differs
//! from canonical while the request stays representable.
//!
//! Only the pairs that both sides can carry are adapted; a result the historical
//! shape cannot represent fails closed rather than being coerced.

use rtd_tg::{InputBotInlineMessageID, MessagesChatInviteJoinResult, Updates};

use crate::error::{HookError, Result};

/// Canonical to profile for `messages.setBotGuestChatResult`: the canonical
/// inline-message-id result becomes the historical `bool` success flag.
///
/// # Errors
///
/// Currently infallible; the signature is fallible for uniformity.
pub fn bot_guest_chat_result(_value: &InputBotInlineMessageID) -> Result<bool> {
    Ok(true)
}

/// Canonical to profile for a chat-invite join result: the canonical `ok` result
/// unwraps to its `Updates`. The webview variant has no historical `Updates`
/// representation and fails closed.
///
/// # Errors
///
/// Returns [`HookError::Unrepresentable`] for the webview variant.
pub fn chat_invite_join_result_to_updates(value: &MessagesChatInviteJoinResult) -> Result<Updates> {
    match value {
        MessagesChatInviteJoinResult::ChatInviteJoinResultOk { updates } => Ok((**updates).clone()),
        MessagesChatInviteJoinResult::ChatInviteJoinResultWebView { .. } => Err(
            HookError::Unrepresentable("chat invite webview result in a historical Updates result"),
        ),
    }
}

/// Profile to canonical for a chat-invite join result: the historical `Updates`
/// becomes the canonical `ok` result.
#[must_use]
pub fn updates_to_chat_invite_join_result(value: Updates) -> MessagesChatInviteJoinResult {
    MessagesChatInviteJoinResult::ChatInviteJoinResultOk {
        updates: Box::new(value),
    }
}

/// Canonical to profile for `account.toggleWebBrowserSettingsException`: the
/// canonical `Updates` result becomes the historical `bool` success flag.
///
/// # Errors
///
/// Currently infallible; the signature is fallible for uniformity.
pub fn web_browser_settings_exception_result(_value: &Updates) -> Result<bool> {
    Ok(true)
}

#[cfg(test)]
mod tests {
    use rtd_tg::{InputBotInlineMessageID, MessagesChatInviteJoinResult, Updates};

    use super::{
        bot_guest_chat_result, chat_invite_join_result_to_updates,
        updates_to_chat_invite_join_result, web_browser_settings_exception_result,
    };
    use crate::error::HookError;

    fn updates() -> Updates {
        Updates::UpdatesTooLong
    }

    #[test]
    fn bot_guest_chat_result_is_a_success_flag() {
        let value = InputBotInlineMessageID::InputBotInlineMessageID64 {
            dc_id: 1,
            owner_id: 2,
            id: 3,
            access_hash: 4,
        };
        assert!(bot_guest_chat_result(&value).unwrap());
    }

    #[test]
    fn chat_invite_result_round_trips_through_updates() {
        let original = updates();
        let result = updates_to_chat_invite_join_result(original.clone());
        assert_eq!(
            chat_invite_join_result_to_updates(&result).unwrap(),
            original
        );
    }

    #[test]
    fn chat_invite_webview_result_fails_closed() {
        let result = MessagesChatInviteJoinResult::ChatInviteJoinResultWebView {
            bot_id: 1,
            query_id: 2,
            users: Vec::new(),
        };
        assert!(matches!(
            chat_invite_join_result_to_updates(&result).unwrap_err(),
            HookError::Unrepresentable(_)
        ));
    }

    #[test]
    fn web_browser_settings_exception_is_a_success_flag() {
        assert!(web_browser_settings_exception_result(&updates()).unwrap());
    }
}
