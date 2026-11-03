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
}
