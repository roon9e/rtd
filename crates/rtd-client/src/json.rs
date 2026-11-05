//! JSON values, constructor-name lookup and pretty-printing.
//!
//! [`JsonValue`] is a small JSON model with a [`Display`](core::fmt::Display)
//! implementation; [`type_name`] resolves a wire id to its constructor name via
//! the generated `types_map`, and [`describe`] renders an object as `name#id`.

use std::collections::BTreeMap;
use std::fmt::{self, Write};

use rtd_tg::Object;

/// A JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    /// `null`.
    Null,
    /// A boolean.
    Bool(bool),
    /// A number.
    Number(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<JsonValue>),
    /// An object (keys are emitted in sorted order for determinism).
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    /// Creates an object from key/value pairs.
    #[must_use]
    pub fn object(entries: impl IntoIterator<Item = (String, JsonValue)>) -> Self {
        Self::Object(entries.into_iter().collect())
    }
}

/// Parses a JSON document into a [`JsonValue`].
///
/// # Errors
///
/// Returns [`JsonError`] when `input` is not valid JSON.
pub fn parse(input: &str) -> Result<JsonValue, JsonError> {
    let mut parser = Parser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    parser.skip_whitespace();
    let value = parser.value()?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(JsonError("trailing characters after JSON value".to_owned()));
    }
    Ok(value)
}

/// A byte-level recursive-descent JSON parser.
struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn skip_whitespace(&mut self) {
        while let Some(&byte) = self.bytes.get(self.pos) {
            if matches!(byte, b' ' | b'\n' | b'\r' | b'\t') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn value(&mut self) -> Result<JsonValue, JsonError> {
        match self.bytes.get(self.pos) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(JsonValue::String(self.string()?)),
            Some(b't') => {
                self.literal("true")?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(JsonValue::Null)
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(JsonError("unexpected JSON token".to_owned())),
        }
    }

    fn object(&mut self) -> Result<JsonValue, JsonError> {
        self.expect(b'{')?;
        let mut entries = BTreeMap::new();
        self.skip_whitespace();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.skip_whitespace();
            let key = self.string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.value()?;
            entries.insert(key, value);
            self.skip_whitespace();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonValue::Object(entries));
                }
                _ => return Err(JsonError("expected ',' or '}'".to_owned())),
            }
        }
    }

    fn array(&mut self) -> Result<JsonValue, JsonError> {
        self.expect(b'[')?;
        let mut values = Vec::new();
        self.skip_whitespace();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(values));
        }
        loop {
            self.skip_whitespace();
            values.push(self.value()?);
            self.skip_whitespace();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonValue::Array(values));
                }
                _ => return Err(JsonError("expected ',' or ']'".to_owned())),
            }
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let byte = *self
                .bytes
                .get(self.pos)
                .ok_or_else(|| JsonError("unterminated string".to_owned()))?;
            self.pos += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escape = *self
                        .bytes
                        .get(self.pos)
                        .ok_or_else(|| JsonError("unterminated escape".to_owned()))?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode_escape()?),
                        _ => return Err(JsonError("invalid escape".to_owned())),
                    }
                }
                other => {
                    // Re-decode the UTF-8 sequence starting at `other`.
                    let start = self.pos - 1;
                    let width = utf8_width(other);
                    let end = start + width;
                    let slice = self
                        .bytes
                        .get(start..end)
                        .ok_or_else(|| JsonError("truncated UTF-8".to_owned()))?;
                    let text = core::str::from_utf8(slice)
                        .map_err(|_| JsonError("invalid UTF-8".to_owned()))?;
                    out.push_str(text);
                    self.pos = end;
                }
            }
        }
    }

    fn unicode_escape(&mut self) -> Result<char, JsonError> {
        let first = self.hex4()?;
        if (0xd800..0xdc00).contains(&first) {
            // High surrogate: expect a low surrogate.
            self.expect(b'\\')?;
            self.expect(b'u')?;
            let second = self.hex4()?;
            let combined = 0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00);
            return char::from_u32(combined)
                .ok_or_else(|| JsonError("invalid surrogate pair".to_owned()));
        }
        char::from_u32(first).ok_or_else(|| JsonError("invalid unicode escape".to_owned()))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let slice = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| JsonError("truncated unicode escape".to_owned()))?;
        let text = core::str::from_utf8(slice)
            .map_err(|_| JsonError("invalid unicode escape".to_owned()))?;
        let value = u32::from_str_radix(text, 16)
            .map_err(|_| JsonError("invalid unicode escape".to_owned()))?;
        self.pos += 4;
        Ok(value)
    }

    fn number(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        while let Some(&byte) = self.bytes.get(self.pos) {
            if matches!(byte, b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = core::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| JsonError("invalid number".to_owned()))?;
        text.parse::<f64>()
            .map(JsonValue::Number)
            .map_err(|_| JsonError(format!("invalid number {text:?}")))
    }

    fn literal(&mut self, literal: &str) -> Result<(), JsonError> {
        if self.bytes[self.pos..].starts_with(literal.as_bytes()) {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(JsonError(format!("expected {literal:?}")))
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), JsonError> {
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(JsonError(format!("expected {:?}", byte as char)))
        }
    }
}

/// Returns the byte width of a UTF-8 sequence from its leading byte.
fn utf8_width(lead: u8) -> usize {
    if lead < 0x80 {
        1
    } else if lead >> 5 == 0b110 {
        2
    } else if lead >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(true) => f.write_str("true"),
            Self::Bool(false) => f.write_str("false"),
            Self::Number(number) => write!(f, "{number}"),
            Self::String(value) => write_string(f, value),
            Self::Array(values) => {
                f.write_char('[')?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        f.write_char(',')?;
                    }
                    write!(f, "{value}")?;
                }
                f.write_char(']')
            }
            Self::Object(entries) => {
                f.write_char('{')?;
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        f.write_char(',')?;
                    }
                    write_string(f, key)?;
                    f.write_char(':')?;
                    write!(f, "{value}")?;
                }
                f.write_char('}')
            }
        }
    }
}

/// Writes a JSON string literal with the required escapes.
fn write_string(f: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    f.write_char('"')?;
    for ch in value.chars() {
        match ch {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\n' => f.write_str("\\n")?,
            '\r' => f.write_str("\\r")?,
            '\t' => f.write_str("\\t")?,
            c if (c as u32) < 0x20 => write!(f, "\\u{:04x}", c as u32)?,
            c => f.write_char(c)?,
        }
    }
    f.write_char('"')
}

/// Returns the constructor name for a wire id, if known.
#[must_use]
pub fn type_name(wire_id: u32) -> Option<&'static str> {
    rtd_tg::types_map()
        .iter()
        .find(|(id, _)| *id == wire_id)
        .map(|(_, name)| *name)
}

/// Renders an object as `name#id`, falling back to `#id` when the name is unknown.
#[must_use]
pub fn describe(object: &Object) -> String {
    let id = object.type_id();
    match type_name(id) {
        Some(name) => format!("{name}#{id:08x}"),
        None => format!("#{id:08x}"),
    }
}

/// A TL-JSON coding error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError(pub String);

impl core::fmt::Display for JsonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for JsonError {}

/// The `@type` field name.
pub const TYPE_FIELD: &str = "@type";

/// Returns the wire id for a constructor name, if known.
#[must_use]
pub fn type_id(name: &str) -> Option<u32> {
    rtd_tg::types_map()
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(id, _)| *id)
}

/// Encodes a TL object to JSON as `{"@type": <name>, "body": <base64>}`.
///
/// The body is the constructor's fields without the wire identifier; decoding
/// looks the identifier up from `@type`, so the value round-trips exactly.
#[must_use]
pub fn encode_object(object: &Object) -> JsonValue {
    let mut writer = rtd_bin::Writer::new();
    object.encode_body(&mut writer);
    let mut entries = BTreeMap::new();
    entries.insert(
        TYPE_FIELD.to_owned(),
        JsonValue::String(describe_name(object.type_id())),
    );
    entries.insert(
        "body".to_owned(),
        JsonValue::String(base64_encode(writer.as_slice())),
    );
    JsonValue::Object(entries)
}

/// Decodes a TL object from its JSON encoding.
///
/// # Errors
///
/// Returns [`JsonError`] when `@type` is missing or unknown, `body` is not
/// valid base64, or the payload does not decode.
pub fn decode_object(value: &JsonValue) -> Result<Object, JsonError> {
    let id = decode_type_id(value)?;
    let bytes = decode_body(value)?;
    let mut decoder = rtd_tg::Decoder::new(&bytes, rtd_tg::DecodeLimits::default());
    Object::decode_with_id(&mut decoder, id).map_err(|error| JsonError(error.to_string()))
}

/// Encodes a TL method to JSON as `{"@type": <name>, "body": <base64>}`.
#[must_use]
pub fn encode_method(method: &rtd_tg::Method) -> JsonValue {
    let mut writer = rtd_bin::Writer::new();
    method.encode_body(&mut writer);
    let mut entries = BTreeMap::new();
    entries.insert(
        TYPE_FIELD.to_owned(),
        JsonValue::String(describe_name(method.type_id())),
    );
    entries.insert(
        "body".to_owned(),
        JsonValue::String(base64_encode(writer.as_slice())),
    );
    JsonValue::Object(entries)
}

/// Decodes a TL method from its JSON encoding.
///
/// # Errors
///
/// Returns [`JsonError`] when `@type` is missing or unknown, `body` is not
/// valid base64, or the payload does not decode.
pub fn decode_method(value: &JsonValue) -> Result<rtd_tg::Method, JsonError> {
    let id = decode_type_id(value)?;
    let bytes = decode_body(value)?;
    let mut decoder = rtd_tg::Decoder::new(&bytes, rtd_tg::DecodeLimits::default());
    rtd_tg::Method::decode_with_id(&mut decoder, id).map_err(|error| JsonError(error.to_string()))
}

/// Renders an RPC error as a JSON object.
#[must_use]
pub fn rpc_error_json(error: &rtd_error::Error) -> JsonValue {
    JsonValue::object([
        (
            TYPE_FIELD.to_owned(),
            JsonValue::String("rpc_error".to_owned()),
        ),
        (
            "error_code".to_owned(),
            JsonValue::Number(f64::from(error.code())),
        ),
        (
            "error_message".to_owned(),
            JsonValue::String(error.message().to_owned()),
        ),
    ])
}

/// Pretty-prints an object using the generated constructor descriptors.
///
/// The generated model exposes constructor identifiers and names (via
/// [`rtd_tg::types_map`]) but not per-field descriptors, so the payload is
/// rendered as its encoded form alongside the resolved name.
#[must_use]
pub fn format(object: &Object) -> String {
    let id = object.type_id();
    let mut writer = rtd_bin::Writer::new();
    object.encode_body(&mut writer);
    format!(
        "{}#{} {{ {} }}",
        describe_name(id),
        id,
        base64_encode(writer.as_slice())
    )
}

/// Returns a constructor's name, or `#id` when unknown.
fn describe_name(id: u32) -> String {
    type_name(id).map_or_else(|| format!("#{id:08x}"), str::to_owned)
}

/// Reads and resolves the `@type` field.
fn decode_type_id(value: &JsonValue) -> Result<u32, JsonError> {
    let JsonValue::Object(entries) = value else {
        return Err(JsonError("expected a JSON object".to_owned()));
    };
    let name = match entries.get(TYPE_FIELD) {
        Some(JsonValue::String(name)) => name,
        _ => return Err(JsonError(format!("missing string {TYPE_FIELD} field"))),
    };
    type_id(name).ok_or_else(|| JsonError(format!("unknown constructor {name:?}")))
}

/// Reads and base64-decodes the `body` field.
fn decode_body(value: &JsonValue) -> Result<Vec<u8>, JsonError> {
    let JsonValue::Object(entries) = value else {
        return Err(JsonError("expected a JSON object".to_owned()));
    };
    match entries.get("body") {
        Some(JsonValue::String(body)) => {
            base64_decode(body).ok_or_else(|| JsonError("invalid base64 body".to_owned()))
        }
        _ => Err(JsonError("missing string \"body\" field".to_owned())),
    }
}

/// Encodes `data` as standard padded base64.
#[must_use]
pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from);
        let b2 = chunk.get(2).copied().map(u32::from);
        let triple = (b0 << 16) | (b1.unwrap_or(0) << 8) | b2.unwrap_or(0);
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if b1.is_some() {
            ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if b2.is_some() {
            ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Decodes standard base64 (padding optional).
#[must_use]
pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{JsonValue, type_name};

    #[test]
    fn json_values_render_canonically() {
        let value = JsonValue::Object(BTreeMap::from([
            ("a".to_owned(), JsonValue::Number(1.0)),
            (
                "b".to_owned(),
                JsonValue::Array(vec![JsonValue::Bool(true), JsonValue::Null]),
            ),
            ("s".to_owned(), JsonValue::String("x\"y".to_owned())),
        ]));
        assert_eq!(value.to_string(), r#"{"a":1,"b":[true,null],"s":"x\"y"}"#);
    }

    #[test]
    fn type_names_resolve_from_the_generated_map() {
        let (id, name) = rtd_tg::types_map()[0];
        assert_eq!(type_name(id), Some(name));
    }

    #[test]
    fn objects_round_trip_through_json() {
        use rtd_tg::Bool;
        use rtd_tg::Object;

        let object = Object::Bool(Box::new(Bool::BoolTrue));
        let encoded = super::encode_object(&object);
        assert_eq!(
            encoded,
            super::JsonValue::Object(std::collections::BTreeMap::from([
                (
                    super::TYPE_FIELD.to_owned(),
                    super::JsonValue::String("boolTrue".to_owned())
                ),
                ("body".to_owned(), super::JsonValue::String(String::new())),
            ]))
        );
        assert_eq!(super::decode_object(&encoded).unwrap(), object);
    }

    #[test]
    fn unknown_types_and_bad_bodies_are_rejected() {
        let unknown = super::JsonValue::Object(std::collections::BTreeMap::from([(
            super::TYPE_FIELD.to_owned(),
            super::JsonValue::String("notAConstructor".to_owned()),
        )]));
        assert!(super::decode_object(&unknown).is_err());

        let bad_body = super::JsonValue::Object(std::collections::BTreeMap::from([
            (
                super::TYPE_FIELD.to_owned(),
                super::JsonValue::String("boolTrue".to_owned()),
            ),
            (
                "body".to_owned(),
                super::JsonValue::String("!!!!".to_owned()),
            ),
        ]));
        assert!(super::decode_object(&bad_body).is_err());
    }

    #[test]
    fn parses_json_text() {
        let value = super::parse(r#"{"s":"x\"y","a":[1,true,null],"u":"\u0041\uD83D\uDE00"}"#)
            .expect("parse");
        let JsonValue::Object(entries) = value else {
            panic!("expected object");
        };
        assert_eq!(
            entries.get("s"),
            Some(&JsonValue::String("x\"y".to_owned()))
        );
        assert_eq!(
            entries.get("a"),
            Some(&JsonValue::Array(vec![
                JsonValue::Number(1.0),
                JsonValue::Bool(true),
                JsonValue::Null,
            ]))
        );
        assert_eq!(
            entries.get("u"),
            Some(&JsonValue::String("A\u{1F600}".to_owned()))
        );
        assert!(super::parse("{").is_err());
        assert!(super::parse("nope").is_err());
    }
}
