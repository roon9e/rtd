//! Reply-markup builders.
//!
//! [`inline_keyboard`] builds a `replyInlineMarkup` from rows of buttons.

use rtd_tg::{
    ButtonType, InlineButtonType, KeyboardButton, KeyboardButtonRow, KeyboardInlineButton,
    KeyboardInlineButtonRow, ReplyMarkup,
};

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

/// The action a reply-keyboard button performs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ReplyButtonKind {
    /// A plain text button.
    #[default]
    Text,
    /// Requests the user's phone number.
    RequestPhone,
    /// Requests the user's location.
    RequestGeoLocation,
    /// Requests a poll; `quiz` selects a quiz poll when set.
    RequestPoll {
        /// Whether the requested poll is a quiz.
        quiz: Option<bool>,
    },
    /// Opens a simple web view.
    SimpleWebView {
        /// The URL to open.
        url: String,
    },
}

/// A reply-keyboard button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyButton {
    /// The button label.
    pub text: String,
    /// The button action.
    pub kind: ReplyButtonKind,
}

impl ReplyButton {
    /// Creates a plain text button.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: ReplyButtonKind::Text,
        }
    }

    /// Creates a button with an explicit action.
    #[must_use]
    pub fn new(text: impl Into<String>, kind: ReplyButtonKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }

    /// Converts the button into its wire form.
    #[must_use]
    pub fn into_button(self) -> KeyboardButton {
        let type_ = match self.kind {
            ReplyButtonKind::Text => ButtonType::ButtonTypeDefault,
            ReplyButtonKind::RequestPhone => ButtonType::ButtonTypeRequestPhone,
            ReplyButtonKind::RequestGeoLocation => ButtonType::ButtonTypeRequestGeoLocation,
            ReplyButtonKind::RequestPoll { quiz } => ButtonType::ButtonTypeRequestPoll {
                flags: if quiz.is_some() { 1 } else { 0 },
                quiz,
            },
            ReplyButtonKind::SimpleWebView { url } => ButtonType::ButtonTypeSimpleWebView { url },
        };
        KeyboardButton {
            flags: 0,
            style: None,
            text: self.text,
            type_: Box::new(type_),
        }
    }
}

/// Options for a reply keyboard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplyKeyboardOptions {
    /// Requests the keyboard be resized to fit.
    pub resize: bool,
    /// Hides the keyboard after a single use.
    pub single_use: bool,
    /// Shows the keyboard only for the users mentioned in the message.
    pub selective: bool,
    /// Keeps the keyboard visible when the chat is opened.
    pub persistent: bool,
    /// Requests a reply from the user.
    pub force_reply: bool,
    /// A placeholder shown in the input field.
    pub placeholder: Option<String>,
}

/// Builds a reply keyboard markup from rows of buttons.
#[must_use]
pub fn reply_keyboard(rows: Vec<Vec<ReplyButton>>, options: ReplyKeyboardOptions) -> ReplyMarkup {
    let rows = rows
        .into_iter()
        .map(|row| KeyboardButtonRow {
            buttons: row.into_iter().map(ReplyButton::into_button).collect(),
        })
        .collect();
    ReplyMarkup::ReplyKeyboardMarkup {
        flags: 0,
        resize: options.resize,
        single_use: options.single_use,
        selective: options.selective,
        persistent: options.persistent,
        force_reply: options.force_reply,
        rows,
        placeholder: options.placeholder,
    }
}

/// Builds a hidden-keyboard markup (`replyKeyboardHide`).
#[must_use]
pub fn reply_keyboard_hide(selective: bool) -> ReplyMarkup {
    ReplyMarkup::ReplyKeyboardHide {
        flags: 0,
        selective,
    }
}

/// Builds a force-reply markup (`replyKeyboardForceReply`).
#[must_use]
pub fn reply_keyboard_force_reply(
    single_use: bool,
    selective: bool,
    placeholder: Option<String>,
) -> ReplyMarkup {
    ReplyMarkup::ReplyKeyboardForceReply {
        flags: 0,
        single_use,
        selective,
        placeholder,
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

    #[test]
    fn builds_reply_keyboard_with_options() {
        use super::{ReplyButton, ReplyButtonKind, ReplyKeyboardOptions, reply_keyboard};

        let markup = reply_keyboard(
            vec![
                vec![ReplyButton::text("A")],
                vec![ReplyButton::new("B", ReplyButtonKind::RequestPhone)],
            ],
            ReplyKeyboardOptions {
                resize: true,
                persistent: true,
                ..ReplyKeyboardOptions::default()
            },
        );
        match markup {
            ReplyMarkup::ReplyKeyboardMarkup {
                resize,
                persistent,
                rows,
                ..
            } => {
                assert!(resize && persistent);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[1].buttons[0].text, "B");
            }
            _ => panic!("expected replyKeyboardMarkup"),
        }
    }

    #[test]
    fn hide_and_force_reply_markups() {
        use super::{reply_keyboard_force_reply, reply_keyboard_hide};

        assert!(matches!(
            reply_keyboard_hide(true),
            ReplyMarkup::ReplyKeyboardHide {
                selective: true,
                ..
            }
        ));
        assert!(matches!(
            reply_keyboard_force_reply(false, false, None),
            ReplyMarkup::ReplyKeyboardForceReply { .. }
        ));
    }
}
