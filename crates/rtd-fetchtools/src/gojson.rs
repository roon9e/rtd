//! A small value model and renderer that reproduces Go's `encoding/json`.
//!
//! The Go tools marshal structs with `json.MarshalIndent(struct, "", indent)`.
//! To keep the Rust artifacts byte-for-byte identical the renderer mirrors Go's
//! rules: object keys keep the struct's declaration order, integers print
//! verbatim, and strings use Go's escaping (which HTML-escapes `<`, `>`, `&`
//! and the line separators `U+2028`/`U+2029`). Empty objects and arrays render
//! as `{}` and `[]`.

use std::fmt::Write as _;

/// A JSON value with ordered object fields.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// JSON `null`.
    Null,
    /// JSON `true`/`false`.
    Bool(bool),
    /// A signed integer.
    Int(i64),
    /// An unsigned integer.
    Uint(u64),
    /// A floating-point number.
    Float(f64),
    /// A string.
    Str(String),
    /// An array.
    Array(Vec<Value>),
    /// An object, with fields in emission order.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// Builds an object from `&str` fields.
    #[must_use]
    pub fn object(fields: Vec<(&str, Value)>) -> Self {
        Self::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Self::Uint(value as u64)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Str(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(value: Vec<T>) -> Self {
        Self::Array(value.into_iter().map(Into::into).collect())
    }
}

impl Value {
    /// Renders the value with `json.MarshalIndent` semantics (`indent` per level).
    #[must_use]
    pub fn render(&self, indent: &str) -> String {
        let mut out = String::new();
        self.write(&mut out, indent, 0);
        out
    }

    fn write(&self, out: &mut String, indent: &str, depth: usize) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(true) => out.push_str("true"),
            Self::Bool(false) => out.push_str("false"),
            Self::Int(value) => {
                let _ = write!(out, "{value}");
            }
            Self::Uint(value) => {
                let _ = write!(out, "{value}");
            }
            Self::Float(value) => out.push_str(&format_float(*value)),
            Self::Str(value) => escape_json_string(value, out),
            Self::Array(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push('\n');
                    push_indent(out, indent, depth + 1);
                    item.write(out, indent, depth + 1);
                }
                out.push('\n');
                push_indent(out, indent, depth);
                out.push(']');
            }
            Self::Object(fields) => {
                if fields.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (index, (key, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push('\n');
                    push_indent(out, indent, depth + 1);
                    escape_json_string(key, out);
                    out.push_str(": ");
                    value.write(out, indent, depth + 1);
                }
                out.push('\n');
                push_indent(out, indent, depth);
                out.push('}');
            }
        }
    }
}

/// Pushes `indent` repeated `depth` times.
fn push_indent(out: &mut String, indent: &str, depth: usize) {
    for _ in 0..depth {
        out.push_str(indent);
    }
}

/// Escapes a string the way Go's `encoding/json` does for HTML-safe output.
pub fn escape_json_string(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            ch if (ch as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

/// Formats a float using Go's `encoding/json` conventions.
///
/// Go uses `'f'` (no exponent) for values in `[1e-6, 1e21)` and exponential
/// notation outside that, with the shortest representation that round-trips.
/// The seed data only ever carries small durations, so the `'f'` path is the one
/// that matters; the exponential path uses Rust's shortest form.
#[must_use]
pub fn format_float(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if !value.is_finite() {
        return "null".to_owned();
    }
    let abs = value.abs();
    if !(1e-6..1e21).contains(&abs) {
        return format!("{value:e}");
    }
    format!("{value}")
}

#[cfg(test)]
mod tests {
    use super::Value;

    #[test]
    fn renders_like_go_marshal_indent() {
        let people = Value::object(vec![(
            "countries",
            Value::Array(vec![
                Value::object(vec![
                    ("iso2", "AB".into()),
                    ("default_name", "A<b>&".into()),
                    ("name", "x".into()),
                    (
                        "country_codes",
                        Value::Array(vec![
                            Value::object(vec![
                                ("country_code", "1".into()),
                                ("prefixes", vec!["1", "2"].into()),
                            ]),
                            Value::object(vec![("country_code", "2".into())]),
                        ]),
                    ),
                ]),
                Value::object(vec![
                    ("iso2", "CD".into()),
                    ("default_name", "cd".into()),
                    ("hidden", true.into()),
                    ("country_codes", Value::Array(vec![])),
                ]),
            ]),
        )]);
        let expected = "{\n  \"countries\": [\n    {\n      \"iso2\": \"AB\",\n      \"default_name\": \"A\\u003cb\\u003e\\u0026\",\n      \"name\": \"x\",\n      \"country_codes\": [\n        {\n          \"country_code\": \"1\",\n          \"prefixes\": [\n            \"1\",\n            \"2\"\n          ]\n        },\n        {\n          \"country_code\": \"2\"\n        }\n      ]\n    },\n    {\n      \"iso2\": \"CD\",\n      \"default_name\": \"cd\",\n      \"hidden\": true,\n      \"country_codes\": []\n    }\n  ]\n}";
        assert_eq!(people.render("  "), expected);
    }

    #[test]
    fn formats_floats_like_go() {
        assert_eq!(super::format_float(5.0), "5");
        assert_eq!(super::format_float(0.1), "0.1");
        assert_eq!(super::format_float(12.34), "12.34");
        assert_eq!(super::format_float(-0.0), "0");
    }
}
