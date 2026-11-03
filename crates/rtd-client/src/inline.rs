//! Inline-result builders for bots.
//!
//! [`set_inline_bot_results`] answers an inline query with a list of results
//! built by [`article`].

use rtd_tg::{
    InputBotInlineMessage, InputBotInlineResult, MessagesSetInlineBotResults, Method, Object,
};

use crate::client::Client;
use crate::error::Result;
use crate::transport::Transport;

/// Builds an inline message that sends `text`.
#[must_use]
pub fn text_message(text: &str) -> InputBotInlineMessage {
    InputBotInlineMessage::InputBotInlineMessageText {
        flags: 0,
        no_webpage: false,
        invert_media: false,
        message: text.to_owned(),
        entities: None,
        reply_markup: None,
    }
}

/// Builds an `article` inline result with `id`, `title` and `text`.
#[must_use]
pub fn article(id: &str, title: &str, text: &str) -> InputBotInlineResult {
    InputBotInlineResult::InputBotInlineResult {
        flags: 0,
        id: id.to_owned(),
        type_: "article".to_owned(),
        title: Some(title.to_owned()),
        description: None,
        url: None,
        thumb: None,
        content: None,
        send_message: Box::new(text_message(text)),
    }
}

/// Answers an inline query with `results` (`messages.setInlineBotResults`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn set_inline_bot_results<T: Transport>(
    client: &mut Client<T>,
    query_id: i64,
    results: Vec<InputBotInlineResult>,
    cache_time: i32,
) -> Result<Object> {
    let query = Method::MessagesSetInlineBotResults(Box::new(MessagesSetInlineBotResults {
        flags: 0,
        gallery: false,
        private: false,
        query_id,
        results,
        cache_time,
        next_offset: None,
        switch_pm: None,
        switch_webview: None,
    }));
    client.invoke(query)
}

#[cfg(test)]
mod tests {
    use rtd_tg::InputBotInlineResult;

    use super::article;

    #[test]
    fn builds_an_article() {
        let result = article("0", "Title", "Body");
        match result {
            InputBotInlineResult::InputBotInlineResult {
                id, type_, title, ..
            } => {
                assert_eq!(id, "0");
                assert_eq!(type_, "article");
                assert_eq!(title.as_deref(), Some("Title"));
            }
            _ => panic!("expected inputBotInlineResult"),
        }
    }
}
