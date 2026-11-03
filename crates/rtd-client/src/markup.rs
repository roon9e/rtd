//! Reply-markup builders.
//!
//! [`inline_keyboard`] builds a `replyInlineMarkup` from rows of buttons.

use rtd_tg::{InlineButtonType, KeyboardInlineButton, KeyboardInlineButtonRow, ReplyMarkup};

/// An inline keyboard button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineButton {
    /// A callback button carrying opaque data.
    Callback {
        /// The button text.
        text: String,
        /// The callback data.
        data: Vec<u8>,
    },
    /// A URL button.
    Url {
        /// The button text.
        text: String,
        /// The URL to open.
        url: String,
    },
}

impl InlineButton {
    /// Converts the button into its wire form.
    #[must_use]
    pub fn into_button(self) -> KeyboardInlineButton {
        let (text, type_) = match self {
            Self::Callback { text, data } => (
                text,
                InlineButtonType::InlineButtonTypeCallback {
                    flags: 0,
                    requires_password: false,
                    data,
                },
            ),
            Self::Url { text, url } => (text, InlineButtonType::InlineButtonTypeUrl { url }),
        };
        KeyboardInlineButton {
            flags: 0,
            style: None,
            text,
            type_: Box::new(type_),
        }
    }
}

/// Builds an inline keyboard reply markup from rows of buttons.
#[must_use]
pub fn inline_keyboard(rows: Vec<Vec<InlineButton>>) -> ReplyMarkup {
    let rows = rows
        .into_iter()
        .map(|row| KeyboardInlineButtonRow {
            buttons: row.into_iter().map(InlineButton::into_button).collect(),
        })
        .collect();
    ReplyMarkup::ReplyInlineMarkup {
        flags: 0,
        force_reply: false,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use rtd_tg::ReplyMarkup;

    use super::{InlineButton, inline_keyboard};

    #[test]
    fn builds_inline_rows() {
        let markup = inline_keyboard(vec![
            vec![InlineButton::Callback {
                text: "A".to_owned(),
                data: b"a".to_vec(),
            }],
            vec![InlineButton::Url {
                text: "B".to_owned(),
                url: "https://x".to_owned(),
            }],
        ]);
        match markup {
            ReplyMarkup::ReplyInlineMarkup { rows, .. } => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].buttons.len(), 1);
                assert_eq!(rows[1].buttons[0].text, "B");
            }
            _ => panic!("expected replyInlineMarkup"),
        }
    }
}
