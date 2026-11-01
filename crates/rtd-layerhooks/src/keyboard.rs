//! Legacy keyboard-button type hook.
//!
//! In layers before 230 a `keyboardButton` had no `type` field; its button type
//! was encoded by the constructor variant. The canonical shape carries the type
//! as a separate `ButtonType`, so decoding an older `keyboardButton` must supply
//! the plain default type.

use rtd_tg::{ButtonType, KeyboardButton};

/// Profile to canonical for `keyboardButton`: supply the canonical
/// `ButtonTypeDefault` the historical shape omitted.
#[must_use]
pub fn default_button_type(_value: &KeyboardButton) -> ButtonType {
    ButtonType::ButtonTypeDefault
}

#[cfg(test)]
mod tests {
    use rtd_tg::{ButtonType, KeyboardButton};

    use super::default_button_type;

    #[test]
    fn a_typeless_legacy_button_gets_the_default_type() {
        let value = KeyboardButton {
            flags: 0,
            style: None,
            text: "ok".to_owned(),
            type_: Box::new(ButtonType::ButtonTypeDefault),
        };
        assert_eq!(default_button_type(&value), ButtonType::ButtonTypeDefault);
    }
}
