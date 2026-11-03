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
}
