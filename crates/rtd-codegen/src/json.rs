//! A small, dependency-free JSON reader.
//!
//! The generator only needs to *read* the versioned `manifest.json`, `policy.json`
//! and `rtd.json` inputs. A full JSON library would be overkill, so this module
//! implements the subset of RFC 8259 needed for those files: objects, arrays,
//! strings with escapes, numbers, booleans and `null`. It intentionally has no
//! writer.

/// A parsed JSON value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, stored as `f64`.
    Number(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object. Insertion order is preserved for deterministic iteration.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Returns the value at `key` for an object, or `None` for any other kind.
    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Returns the value at `key`, or an error naming the missing field.
    ///
    /// # Errors
    ///
    /// Returns a message when `self` is not an object or `key` is absent.
    pub(crate) fn field(&self, key: &str) -> Result<&Json, String> {
        self.get(key)
            .ok_or_else(|| format!("missing JSON field {key:?}"))
    }

    /// Borrows the value as a string.
    ///
    /// # Errors
    ///
    /// Returns a message when the value is not a string.
    pub(crate) fn as_str(&self) -> Result<&str, String> {
        match self {
            Json::String(value) => Ok(value),
            other => Err(format!("expected string, found {other:?}")),
        }
    }

    /// Borrows the value as an array.
    ///
    /// # Errors
    ///
    /// Returns a message when the value is not an array.
    pub(crate) fn as_array(&self) -> Result<&[Json], String> {
        match self {
            Json::Array(values) => Ok(values),
            other => Err(format!("expected array, found {other:?}")),
        }
    }

    /// Reads the value as an integer.
    ///
    /// # Errors
    ///
    /// Returns a message when the value is not a number that fits in `i64`.
    pub(crate) fn as_i64(&self) -> Result<i64, String> {
        match self {
            Json::Number(value) if value.fract() == 0.0 => Ok(*value as i64),
            other => Err(format!("expected integer, found {other:?}")),
        }
    }
}

/// Parses a complete JSON document.
///
/// # Errors
///
/// Returns a message describing the first syntax error or trailing data.
pub(crate) fn parse(input: &str) -> Result<Json, String> {
    let mut parser = Parser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    parser.skip_ws();
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(format!("trailing data at byte {}", parser.pos));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(byte) = self.bytes.get(self.pos) {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", byte as char, self.pos))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(_) => self.number(),
            None => Err("unexpected end of input".to_owned()),
        }
    }

    fn literal(&mut self, text: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(text.as_bytes()) {
            self.pos += text.len();
            Ok(value)
        } else {
            Err(format!("invalid literal at byte {}", self.pos))
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Object(entries));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            let value = self.value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.pos)),
            }
        }
        Ok(Json::Object(entries))
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut values = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Array(values));
        }
        loop {
            self.skip_ws();
            values.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.pos)),
            }
        }
        Ok(Json::Array(values))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let byte = self
                .bytes
                .get(self.pos)
                .copied()
                .ok_or_else(|| "unterminated string".to_owned())?;
            self.pos += 1;
            match byte {
                b'"' => break,
                b'\\' => {
                    let escape = self
                        .bytes
                        .get(self.pos)
                        .copied()
                        .ok_or_else(|| "unterminated escape".to_owned())?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let code = self.hex4()?;
                            if (0xD800..=0xDBFF).contains(&code) {
                                // Surrogate pair: expect a following \uXXXX low surrogate.
                                if self.peek() == Some(b'\\') {
                                    self.pos += 1;
                                    self.expect(b'u')?;
                                    let low = self.hex4()?;
                                    let combined =
                                        0x1_0000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                                    out.push(
                                        char::from_u32(combined)
                                            .ok_or_else(|| "invalid surrogate pair".to_owned())?,
                                    );
                                } else {
                                    return Err("lone high surrogate".to_owned());
                                }
                            } else {
                                out.push(
                                    char::from_u32(code)
                                        .ok_or_else(|| "invalid code point".to_owned())?,
                                );
                            }
                        }
                        other => return Err(format!("invalid escape \\{}", other as char)),
                    }
                }
                _ => {
                    // Copy the raw UTF-8 byte; revalidate at the end.
                    out.push(byte as char);
                }
            }
        }
        // The byte-wise copy above is valid for ASCII; re-decode when needed.
        if out.is_ascii() {
            return Ok(out);
        }
        Err("non-ASCII string handling is not required for the schema inputs".to_owned())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let slice = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| "truncated \\u escape".to_owned())?;
        let text = std::str::from_utf8(slice).map_err(|_| "invalid \\u escape".to_owned())?;
        let value = u32::from_str_radix(text, 16).map_err(|_| "invalid \\u escape".to_owned())?;
        self.pos += 4;
        Ok(value)
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(byte) = self.peek() {
            if byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "invalid number".to_owned())?;
        text.parse::<f64>()
            .map(Json::Number)
            .map_err(|_| format!("invalid number {text:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Json, parse};

    #[test]
    fn parses_nested_document() {
        let value = parse(r#"{"a":[1,2,{"b":true}],"c":null,"d":"x"}"#).unwrap();
        assert_eq!(value.get("a").unwrap().as_array().unwrap().len(), 3);
        assert_eq!(value.get("d").unwrap().as_str().unwrap(), "x");
        assert!(matches!(value.get("c").unwrap(), Json::Null));
    }

    #[test]
    fn rejects_trailing_data() {
        assert!(parse("{} {}").is_err());
    }

    #[test]
    fn handles_unicode_escape() {
        let value = parse(r#""\u003e""#).unwrap();
        assert_eq!(value.as_str().unwrap(), ">");
    }
}
