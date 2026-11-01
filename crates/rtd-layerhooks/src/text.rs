//! Verification-description text hooks.
//!
//! Layers before 230 carried a bot's verification description as a plain
//! `string`; the canonical layer carries a [`TextWithEntities`]. The historical
//! shape has no entity field, so only a lossless plain-text projection is
//! accepted — a value with entities fails closed rather than dropping them.

use rtd_tg::TextWithEntities;

use crate::error::{HookError, Result};

/// Projects a verification description to its plain text, failing closed when
/// the value carries entities the historical shape cannot represent.
///
/// # Errors
///
/// Returns [`HookError::Unrepresentable`] when `value` has any text entity.
pub fn verification_plain_text(value: &TextWithEntities) -> Result<String> {
    if !value.entities.is_empty() {
        return Err(HookError::Unrepresentable(
            "historical bot verification text entities",
        ));
    }
    Ok(value.text.clone())
}

/// Canonical to profile: `botVerification.description` (`TextWithEntities`)
/// becomes the historical `string`.
///
/// # Errors
///
/// As [`verification_plain_text`].
pub fn bot_verification_text_encode(value: &TextWithEntities) -> Result<String> {
    verification_plain_text(value)
}

/// Profile to canonical: the historical `string` becomes a `TextWithEntities`
/// with no entities.
#[must_use]
pub fn bot_verification_text_decode(value: &str) -> TextWithEntities {
    TextWithEntities {
        text: value.to_owned(),
        entities: Vec::new(),
    }
}

/// Canonical to profile for an optional description: returns `None` when the
/// field is absent.
///
/// # Errors
///
/// As [`verification_plain_text`] when the field is present.
pub fn optional_text_encode(present: bool, value: &TextWithEntities) -> Result<Option<String>> {
    if present {
        verification_plain_text(value).map(Some)
    } else {
        Ok(None)
    }
}

/// Profile to canonical for an optional description: returns `None` when the
/// field is absent.
#[must_use]
pub fn optional_text_decode(present: bool, value: &str) -> Option<TextWithEntities> {
    if present {
        Some(bot_verification_text_decode(value))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use rtd_tg::{MessageEntity, TextWithEntities};

    use super::{
        bot_verification_text_decode, bot_verification_text_encode, optional_text_decode,
        optional_text_encode, verification_plain_text,
    };
    use crate::error::HookError;

    fn plain(text: &str) -> TextWithEntities {
        TextWithEntities {
            text: text.to_owned(),
            entities: Vec::new(),
        }
    }

    #[test]
    fn plain_text_projects_losslessly() {
        let value = plain("verified");
        assert_eq!(verification_plain_text(&value).unwrap(), "verified");
        assert_eq!(bot_verification_text_encode(&value).unwrap(), "verified");
    }

    #[test]
    fn entities_fail_closed() {
        let value = TextWithEntities {
            text: "verified".to_owned(),
            entities: vec![MessageEntity::MessageEntityUnknown {
                offset: 0,
                length: 1,
            }],
        };
        assert_eq!(
            verification_plain_text(&value).unwrap_err(),
            HookError::Unrepresentable("historical bot verification text entities")
        );
        assert!(bot_verification_text_encode(&value).is_err());
    }

    #[test]
    fn decode_builds_text_without_entities() {
        assert_eq!(bot_verification_text_decode("hello"), plain("hello"));
    }

    #[test]
    fn optional_field_respects_presence() {
        assert_eq!(optional_text_encode(false, &plain("x")).unwrap(), None);
        assert_eq!(
            optional_text_encode(true, &plain("x")).unwrap(),
            Some("x".to_owned())
        );
        assert_eq!(optional_text_decode(false, "x"), None);
        assert_eq!(optional_text_decode(true, "x"), Some(plain("x")));
    }
}
