//! Structural pretty-printing of TL objects.
//!
//! [`format`] renders any [`Object`] as its constructor and named fields by
//! using the generated `Debug` representation, which mirrors `td`'s `tdp`. Byte
//! fields render as decimal lists (base64 is available through
//! [`crate::json::encode_object`]).

use rtd_tg::Object;

/// Pretty-prints `object` as a multi-line, field-by-field dump.
#[must_use]
pub fn format(object: &Object) -> String {
    format!("{object:#?}")
}

/// Pretty-prints `object` on a single line.
#[must_use]
pub fn compact(object: &Object) -> String {
    format!("{object:?}")
}

/// Returns the `name#id` label for `object`.
#[must_use]
pub fn label(object: &Object) -> String {
    crate::json::describe(object)
}

#[cfg(test)]
mod tests {
    use rtd_tg::{Bool, InputPeer, Object};

    #[test]
    fn renders_constructor_and_fields() {
        let object = Object::Bool(Box::new(Bool::BoolTrue));
        assert!(super::format(&object).contains("BoolTrue"));
        assert!(super::compact(&object).contains("BoolTrue"));
        assert!(super::label(&object).starts_with("boolTrue#"));
    }

    #[test]
    fn renders_named_fields() {
        let object = Object::InputPeer(Box::new(InputPeer::InputPeerChat { chat_id: 7 }));
        let rendered = super::format(&object);
        assert!(rendered.contains("InputPeerChat"));
        assert!(rendered.contains("chat_id"));
    }
}
