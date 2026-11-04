//! Message entity parsing.
//!
//! [`parse_html`] converts an HTML subset (`<b>`, `<i>`, `<u>`, `<s>`, `<code>`,
//! `<pre>`, `<a href>`, `<tg-spoiler>`, `<blockquote>`) into plain text plus
//! [`MessageEntity`] offsets, using UTF-16 code units as required by Telegram.

use rtd_tg::MessageEntity;

/// An entity currently open in the input.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Open {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Pre(String),
    Spoiler,
    TextUrl(String),
    Blockquote,
}

impl Open {
    /// Returns the tag name(s) that close this entity.
    fn names(&self) -> &'static [&'static str] {
        match self {
            Self::Bold => &["b", "strong"],
            Self::Italic => &["i", "em"],
            Self::Underline => &["u", "ins"],
            Self::Strike => &["s", "strike", "del"],
            Self::Code => &["code"],
            Self::Pre(_) => &["pre"],
            Self::Spoiler => &["tg-spoiler", "spoiler"],
            Self::TextUrl(_) => &["a"],
            Self::Blockquote => &["blockquote"],
        }
    }
}

/// Parses an HTML subset into plain text and its entities.
#[must_use]
pub fn parse_html(html: &str) -> (String, Vec<MessageEntity>) {
    let mut text = String::new();
    let mut entities = Vec::new();
    let mut stack: Vec<(Open, i32)> = Vec::new();
    let mut offset: i32 = 0;
    let bytes = html.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'<'
            && let Some(rel) = html[index..].find('>')
        {
            let tag = &html[index + 1..index + rel];
            handle_tag(tag, &mut stack, &mut entities, offset);
            index += rel + 1;
            continue;
        }
        if bytes[index] == b'&'
            && let Some((decoded, consumed)) = decode_entity(&html[index..])
        {
            offset += decoded.encode_utf16().count() as i32;
            text.push_str(decoded);
            index += consumed;
            continue;
        }
        let ch = html[index..].chars().next().expect("valid boundary");
        text.push(ch);
        offset += ch.len_utf16() as i32;
        index += ch.len_utf8();
    }

    // Close any entities left open at the end.
    while let Some((open, start)) = stack.pop() {
        entities.push(build(open, start, offset - start));
    }
    (text, entities)
}

/// A markdown entity currently open.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mark {
    Bold,
    Italic,
    Strike,
    Spoiler,
    Code,
}

impl Mark {
    fn build(&self, offset: i32, length: i32) -> MessageEntity {
        match self {
            Self::Bold => MessageEntity::MessageEntityBold { offset, length },
            Self::Italic => MessageEntity::MessageEntityItalic { offset, length },
            Self::Strike => MessageEntity::MessageEntityStrike { offset, length },
            Self::Spoiler => MessageEntity::MessageEntitySpoiler { offset, length },
            Self::Code => MessageEntity::MessageEntityCode { offset, length },
        }
    }
}

/// Parses a Markdown subset into plain text and its entities.
///
/// Supports `**bold**`, `__italic__`, `~~strike~~`, `||spoiler||`, `` `code` ``
/// and `[text](url)` links.
#[must_use]
pub fn parse_markdown(markdown: &str) -> (String, Vec<MessageEntity>) {
    let mut text = String::new();
    let mut entities = Vec::new();
    let mut stack: Vec<(Mark, i32)> = Vec::new();
    let mut offset: i32 = 0;
    let bytes = markdown.as_bytes();
    let mut index = 0usize;

    // Pushes one character, advancing the UTF-16 offset.
    fn push(text: &mut String, offset: &mut i32, ch: char) {
        text.push(ch);
        *offset += ch.len_utf16() as i32;
    }

    while index < bytes.len() {
        // Backslash escapes the next character.
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            let ch = markdown[index + 1..].chars().next().expect("boundary");
            push(&mut text, &mut offset, ch);
            index += 1 + ch.len_utf8();
            continue;
        }

        if let Some((mark, width)) = delimiter(&bytes[index..]) {
            if matches!(stack.last(), Some((open, _)) if open == &mark) {
                let (open, start) = stack.pop().expect("non-empty");
                entities.push(open.build(start, offset - start));
            } else {
                stack.push((mark, offset));
            }
            index += width;
            continue;
        }

        if bytes[index] == b'['
            && let Some((label, url, consumed)) = parse_link(&markdown[index..])
        {
            let start = offset;
            for ch in label.chars() {
                push(&mut text, &mut offset, ch);
            }
            entities.push(MessageEntity::MessageEntityTextUrl {
                offset: start,
                length: offset - start,
                url,
            });
            index += consumed;
            continue;
        }

        let ch = markdown[index..].chars().next().expect("boundary");
        push(&mut text, &mut offset, ch);
        index += ch.len_utf8();
    }

    while let Some((mark, start)) = stack.pop() {
        entities.push(mark.build(start, offset - start));
    }
    (text, entities)
}

/// Matches an opening/closing delimiter at the start of `bytes`.
fn delimiter(bytes: &[u8]) -> Option<(Mark, usize)> {
    if bytes.starts_with(b"**") {
        Some((Mark::Bold, 2))
    } else if bytes.starts_with(b"__") {
        Some((Mark::Italic, 2))
    } else if bytes.starts_with(b"~~") {
        Some((Mark::Strike, 2))
    } else if bytes.starts_with(b"||") {
        Some((Mark::Spoiler, 2))
    } else if bytes.first() == Some(&b'`') {
        Some((Mark::Code, 1))
    } else {
        None
    }
}

/// Parses a `[label](url)` link, returning the label, url and consumed bytes.
fn parse_link(input: &str) -> Option<(String, String, usize)> {
    let close = input.find(']')?;
    let label = input[1..close].to_owned();
    let after = &input[close + 1..];
    let open = after.strip_prefix('(')?;
    let end = open.find(')')?;
    let url = open[..end].to_owned();
    Some((label, url, close + 1 + 1 + end + 1))
}

/// Handles one tag (without the angle brackets).
fn handle_tag(
    tag: &str,
    stack: &mut Vec<(Open, i32)>,
    entities: &mut Vec<MessageEntity>,
    offset: i32,
) {
    if let Some(name) = tag.strip_prefix('/') {
        let name = name.trim().to_ascii_lowercase();
        if let Some((open, start)) = stack.pop()
            && open.names().contains(&name.as_str())
        {
            entities.push(build(open, start, offset - start));
        }
        return;
    }
    let name = tag
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match name.as_str() {
        "b" | "strong" => stack.push((Open::Bold, offset)),
        "i" | "em" => stack.push((Open::Italic, offset)),
        "u" | "ins" => stack.push((Open::Underline, offset)),
        "s" | "strike" | "del" => stack.push((Open::Strike, offset)),
        "code" => stack.push((Open::Code, offset)),
        "pre" => stack.push((
            Open::Pre(attribute(tag, "language").unwrap_or_default()),
            offset,
        )),
        "tg-spoiler" | "spoiler" => stack.push((Open::Spoiler, offset)),
        "a" => stack.push((
            Open::TextUrl(attribute(tag, "href").unwrap_or_default()),
            offset,
        )),
        "blockquote" => stack.push((Open::Blockquote, offset)),
        _ => {}
    }
}

/// Builds an entity from an open tag and its span.
fn build(open: Open, offset: i32, length: i32) -> MessageEntity {
    match open {
        Open::Bold => MessageEntity::MessageEntityBold { offset, length },
        Open::Italic => MessageEntity::MessageEntityItalic { offset, length },
        Open::Underline => MessageEntity::MessageEntityUnderline { offset, length },
        Open::Strike => MessageEntity::MessageEntityStrike { offset, length },
        Open::Code => MessageEntity::MessageEntityCode { offset, length },
        Open::Pre(language) => MessageEntity::MessageEntityPre {
            offset,
            length,
            language,
        },
        Open::Spoiler => MessageEntity::MessageEntitySpoiler { offset, length },
        Open::TextUrl(url) => MessageEntity::MessageEntityTextUrl {
            offset,
            length,
            url,
        },
        Open::Blockquote => MessageEntity::MessageEntityBlockquote {
            flags: 0,
            collapsed: false,
            offset,
            length,
        },
    }
}

/// Extracts a `name="value"` attribute from a tag body.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let marker = format!("{name}=");
    let start = tag.find(&marker)? + marker.len();
    let rest = tag[start..].trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        return Some(rest[..end].to_owned());
    }
    let value = &rest[1..];
    let end = value.find(quote)?;
    Some(value[..end].to_owned())
}

/// Decodes an HTML entity at the start of `input`, returning the value and the
/// number of consumed bytes.
fn decode_entity(input: &str) -> Option<(&'static str, usize)> {
    const ENTITIES: [(&str, &str); 5] = [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp;", "&"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ];
    for (encoded, decoded) in ENTITIES {
        if input.starts_with(encoded) {
            return Some((decoded, encoded.len()));
        }
    }
    None
}

/// A style applied to a span of text by [`EntityBuilder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityKind {
    /// Bold text.
    Bold,
    /// Italic text.
    Italic,
    /// Underlined text.
    Underline,
    /// Struck-through text.
    Strike,
    /// Spoiler text.
    Spoiler,
    /// Monospaced inline code.
    Code,
    /// A preformatted block with an optional language.
    Pre {
        /// The language of the block.
        language: String,
    },
    /// A link whose text differs from its target.
    TextUrl {
        /// The link target.
        url: String,
    },
    /// A bare URL.
    Url,
    /// An email address.
    Email,
    /// A `@mention`.
    Mention,
    /// A `#hashtag`.
    Hashtag,
    /// A `$cashtag`.
    Cashtag,
    /// A `/botcommand`.
    BotCommand,
    /// A block quote.
    Blockquote {
        /// Whether the quote is collapsed by default.
        collapsed: bool,
    },
}

impl EntityKind {
    /// Builds the wire entity for a span.
    fn at(&self, offset: i32, length: i32) -> MessageEntity {
        match self {
            Self::Bold => MessageEntity::MessageEntityBold { offset, length },
            Self::Italic => MessageEntity::MessageEntityItalic { offset, length },
            Self::Underline => MessageEntity::MessageEntityUnderline { offset, length },
            Self::Strike => MessageEntity::MessageEntityStrike { offset, length },
            Self::Spoiler => MessageEntity::MessageEntitySpoiler { offset, length },
            Self::Code => MessageEntity::MessageEntityCode { offset, length },
            Self::Pre { language } => MessageEntity::MessageEntityPre {
                offset,
                length,
                language: language.clone(),
            },
            Self::TextUrl { url } => MessageEntity::MessageEntityTextUrl {
                offset,
                length,
                url: url.clone(),
            },
            Self::Url => MessageEntity::MessageEntityUrl { offset, length },
            Self::Email => MessageEntity::MessageEntityEmail { offset, length },
            Self::Mention => MessageEntity::MessageEntityMention { offset, length },
            Self::Hashtag => MessageEntity::MessageEntityHashtag { offset, length },
            Self::Cashtag => MessageEntity::MessageEntityCashtag { offset, length },
            Self::BotCommand => MessageEntity::MessageEntityBotCommand { offset, length },
            Self::Blockquote { collapsed } => MessageEntity::MessageEntityBlockquote {
                flags: if *collapsed { 1 } else { 0 },
                collapsed: *collapsed,
                offset,
                length,
            },
        }
    }
}

/// Builds a message and its styled spans, tracking UTF-16 offsets.
///
/// Telegram measures entity offsets and lengths in UTF-16 code units, which
/// [`Self::styled`] handles automatically.
#[derive(Debug, Default, Clone)]
pub struct EntityBuilder {
    text: String,
    entities: Vec<MessageEntity>,
    utf16_len: i32,
}

impl EntityBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends unstyled text.
    pub fn plain(&mut self, text: &str) -> &mut Self {
        self.text.push_str(text);
        self.utf16_len += utf16_len(text);
        self
    }

    /// Appends `text` under one entity style.
    pub fn styled(&mut self, text: &str, kind: &EntityKind) -> &mut Self {
        self.styled_with(text, std::slice::from_ref(kind))
    }

    /// Appends `text` under several entity styles at once.
    pub fn styled_with(&mut self, text: &str, kinds: &[EntityKind]) -> &mut Self {
        let offset = self.utf16_len;
        let length = utf16_len(text);
        for kind in kinds {
            self.entities.push(kind.at(offset, length));
        }
        self.text.push_str(text);
        self.utf16_len += length;
        self
    }

    /// Returns the message text built so far.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the entities built so far.
    #[must_use]
    pub fn entities(&self) -> &[MessageEntity] {
        &self.entities
    }

    /// Consumes the builder, returning the text and its entities.
    #[must_use]
    pub fn complete(self) -> (String, Vec<MessageEntity>) {
        (self.text, self.entities)
    }
}

/// Returns the length of `text` in UTF-16 code units.
#[must_use]
pub fn utf16_len(text: &str) -> i32 {
    text.chars().map(|ch| ch.len_utf16() as i32).sum()
}

#[cfg(test)]
mod tests {
    use super::parse_html;
    use rtd_tg::MessageEntity;

    #[test]
    fn parses_nested_entities() {
        let (text, entities) = parse_html("<b>bold <i>and italic</i></b>!");
        assert_eq!(text, "bold and italic!");
        assert!(entities.contains(&MessageEntity::MessageEntityBold {
            offset: 0,
            length: 15,
        }));
        assert!(entities.contains(&MessageEntity::MessageEntityItalic {
            offset: 5,
            length: 10,
        }));
    }

    #[test]
    fn parses_links_and_escapes() {
        let (text, entities) = parse_html(r#"<a href="https://x">a &amp; b</a>"#);
        assert_eq!(text, "a & b");
        assert_eq!(
            entities,
            vec![MessageEntity::MessageEntityTextUrl {
                offset: 0,
                length: 5,
                url: "https://x".to_owned(),
            }]
        );
    }

    #[test]
    fn parses_markdown() {
        use super::parse_markdown;
        let (text, entities) = parse_markdown("**bold** and [link](https://x)");
        assert_eq!(text, "bold and link");
        assert!(entities.contains(&MessageEntity::MessageEntityBold {
            offset: 0,
            length: 4,
        }));
        assert!(entities.contains(&MessageEntity::MessageEntityTextUrl {
            offset: 9,
            length: 4,
            url: "https://x".to_owned(),
        }));
    }

    #[test]
    fn uses_utf16_offsets() {
        // The emoji is two UTF-16 code units, so the bold span starts at 2.
        let (text, entities) = parse_html("\u{1F600}<b>hi</b>");
        assert_eq!(text, "\u{1F600}hi");
        assert_eq!(
            entities,
            vec![MessageEntity::MessageEntityBold {
                offset: 2,
                length: 2,
            }]
        );
    }

    #[test]
    fn entity_builder_tracks_utf16_offsets() {
        use super::{EntityBuilder, EntityKind, utf16_len};

        assert_eq!(utf16_len("\u{1F600}"), 2);

        let mut builder = EntityBuilder::new();
        builder
            .plain("\u{1F600} ")
            .styled("bold", &EntityKind::Bold);
        assert_eq!(builder.text(), "\u{1F600} bold");
        assert_eq!(
            builder.entities(),
            &[MessageEntity::MessageEntityBold {
                offset: 3,
                length: 4,
            }]
        );
    }

    #[test]
    fn entity_builder_applies_several_styles() {
        use super::{EntityBuilder, EntityKind};

        let mut builder = EntityBuilder::new();
        builder.plain("hi ").styled_with(
            "link",
            &[
                EntityKind::Bold,
                EntityKind::TextUrl {
                    url: "https://x".to_owned(),
                },
            ],
        );
        let (text, entities) = builder.complete();
        assert_eq!(text, "hi link");
        assert_eq!(entities.len(), 2);
        assert_eq!(
            entities[0],
            MessageEntity::MessageEntityBold {
                offset: 3,
                length: 4
            }
        );
    }
}
