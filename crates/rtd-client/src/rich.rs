//! Rich-message builders.
//!
//! Telegram rich messages are a tree of [`PageBlock`]s containing [`RichText`].
//! The server can also accept the raw HTML/Markdown form
//! (`inputRichMessageHTML`/`inputRichMessageMarkdown`), which [`RichMessage`]
//! exposes directly.

use rtd_tg::{InputRichMessage, PageBlock, RichText};

/// Builds a plain rich-text node.
#[must_use]
pub fn text(value: impl Into<String>) -> RichText {
    RichText::TextPlain { text: value.into() }
}

/// Builds a bold rich-text node.
#[must_use]
pub fn bold(value: RichText) -> RichText {
    RichText::TextBold {
        text: Box::new(value),
    }
}

/// Builds an italic rich-text node.
#[must_use]
pub fn italic(value: RichText) -> RichText {
    RichText::TextItalic {
        text: Box::new(value),
    }
}

/// Builds a monospaced rich-text node.
#[must_use]
pub fn fixed(value: RichText) -> RichText {
    RichText::TextFixed {
        text: Box::new(value),
    }
}

/// Builds a link rich-text node.
#[must_use]
pub fn link(value: RichText, url: impl Into<String>) -> RichText {
    RichText::TextUrl {
        text: Box::new(value),
        url: url.into(),
        webpage_id: 0,
    }
}

/// Concatenates rich-text nodes.
#[must_use]
pub fn concat(nodes: Vec<RichText>) -> RichText {
    RichText::TextConcat { texts: nodes }
}

/// A rich message to attach to a send or edit.
#[derive(Debug, Clone, PartialEq)]
pub enum RichMessage {
    /// Raw rich HTML, parsed by the server.
    Html(String),
    /// Raw rich Markdown, parsed by the server.
    Markdown(String),
    /// A structured block tree.
    Blocks(Vec<PageBlock>),
}

impl RichMessage {
    /// Creates a rich message from raw HTML.
    #[must_use]
    pub fn html(source: impl Into<String>) -> Self {
        Self::Html(source.into())
    }

    /// Creates a rich message from raw Markdown.
    #[must_use]
    pub fn markdown(source: impl Into<String>) -> Self {
        Self::Markdown(source.into())
    }

    /// Creates a rich message from a block tree.
    #[must_use]
    pub fn blocks(blocks: Vec<PageBlock>) -> Self {
        Self::Blocks(blocks)
    }

    /// Creates a rich message with a single paragraph.
    #[must_use]
    pub fn paragraph(value: RichText) -> Self {
        Self::Blocks(vec![PageBlock::PageBlockParagraph {
            text: Box::new(value),
        }])
    }

    /// Creates a rich message with a single heading of level `1..=6`.
    ///
    /// Levels outside `1..=6` are clamped.
    #[must_use]
    pub fn heading(level: u8, value: RichText) -> Self {
        let block = match level.clamp(1, 6) {
            1 => PageBlock::PageBlockHeading1 {
                text: Box::new(value),
            },
            2 => PageBlock::PageBlockHeading2 {
                text: Box::new(value),
            },
            3 => PageBlock::PageBlockHeading3 {
                text: Box::new(value),
            },
            4 => PageBlock::PageBlockHeading4 {
                text: Box::new(value),
            },
            5 => PageBlock::PageBlockHeading5 {
                text: Box::new(value),
            },
            _ => PageBlock::PageBlockHeading6 {
                text: Box::new(value),
            },
        };
        Self::Blocks(vec![block])
    }

    /// Converts the message into its wire form.
    #[must_use]
    pub fn into_input(self) -> InputRichMessage {
        match self {
            Self::Html(html) => InputRichMessage::InputRichMessageHTML {
                flags: 0,
                rtl: false,
                noautolink: false,
                html,
                files: None,
            },
            Self::Markdown(markdown) => InputRichMessage::InputRichMessageMarkdown {
                flags: 0,
                rtl: false,
                noautolink: false,
                markdown,
                files: None,
            },
            Self::Blocks(blocks) => InputRichMessage::InputRichMessage {
                flags: 0,
                rtl: false,
                noautolink: false,
                blocks,
                photos: None,
                documents: None,
                users: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use rtd_tg::{InputRichMessage, PageBlock, RichText};

    use super::{RichMessage, bold, text};

    #[test]
    fn html_and_markdown_map_to_their_input_variants() {
        assert!(matches!(
            RichMessage::html("<b>x</b>").into_input(),
            InputRichMessage::InputRichMessageHTML { .. }
        ));
        assert!(matches!(
            RichMessage::markdown("**x**").into_input(),
            InputRichMessage::InputRichMessageMarkdown { .. }
        ));
    }

    #[test]
    fn heading_builds_a_block() {
        let message = RichMessage::heading(2, bold(text("Title")));
        match message.into_input() {
            InputRichMessage::InputRichMessage { blocks, .. } => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    PageBlock::PageBlockHeading2 { text } => {
                        assert!(matches!(**text, RichText::TextBold { .. }));
                    }
                    other => panic!("unexpected block: {other:?}"),
                }
            }
            _ => panic!("expected inputRichMessage"),
        }
        // A plain paragraph is `PageBlockParagraph` over `TextBold`.
        let RichMessage::Blocks(blocks) = RichMessage::paragraph(bold(text("p"))) else {
            panic!("expected blocks");
        };
        match &blocks[0] {
            PageBlock::PageBlockParagraph { text } => {
                assert!(matches!(**text, RichText::TextBold { .. }));
            }
            other => panic!("unexpected block: {other:?}"),
        }
    }
}
