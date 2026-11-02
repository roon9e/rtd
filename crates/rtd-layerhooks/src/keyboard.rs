//! Legacy keyboard-button type hooks.
//!
//! In layers before 230 a `keyboardButton` had no `type` field; its button type
//! was encoded by the constructor variant. The canonical shape carries the type
//! as a separate `ButtonType`, so decoding an older `keyboardButton` must supply
//! the plain default type, and only a default-typed button maps back.

use rtd_tg::{ButtonType, KeyboardButton};

use crate::error::Result;

/// Profile to canonical for `keyboardButton`: supply the canonical
/// `ButtonTypeDefault` the historical shape omitted.
#[must_use]
pub fn supply_default_button_type() -> ButtonType {
    ButtonType::ButtonTypeDefault
}

/// Canonical to profile for `keyboardButton`: only a default-typed button maps
/// to the historical typeless button; any other type cannot be represented and
/// is projected away.
///
/// # Errors
///
/// Currently infallible; the signature is fallible for uniformity.
pub fn project_default_button_type(value: &KeyboardButton) -> Result<Option<KeyboardButton>> {
    if matches!(*value.type_, ButtonType::ButtonTypeDefault) {
        Ok(Some(value.clone()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use rtd_tg::{ButtonType, KeyboardButton};

    use super::{project_default_button_type, supply_default_button_type};

    fn button(button_type: ButtonType) -> KeyboardButton {
        KeyboardButton {
            flags: 0,
            style: None,
            text: "ok".to_owned(),
            type_: Box::new(button_type),
        }
    }

    #[test]
    fn a_typeless_legacy_button_gets_the_default_type() {
        assert_eq!(supply_default_button_type(), ButtonType::ButtonTypeDefault);
    }

    #[test]
    fn a_default_typed_button_survives_projection() {
        let value = button(ButtonType::ButtonTypeDefault);
        assert_eq!(project_default_button_type(&value).unwrap(), Some(value));
    }

    #[test]
    fn a_non_default_button_is_projected_away() {
        let value = button(ButtonType::ButtonTypeRequestPhone);
        assert!(project_default_button_type(&value).unwrap().is_none());
    }
}
