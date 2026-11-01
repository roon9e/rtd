//! Verification-description text hooks.
//!
//! Layers before 230 carried a bot's verification description as a plain
//! `string`; the canonical layer carries a [`TextWithEntities`]. The historical
//! shape has no entity field, so only a lossless plain-text projection is
//! accepted — a value with entities fails closed rather than dropping them.
//!
//! These are field-shaped hooks: they convert the field's inner value, and the
//! generated call site handles any surrounding `Option`.

use rtd_tg::TextWithEntities;

use crate::error::{HookError, Result};

/// Canonical to profile: a [`TextWithEntities`] becomes the historical plain
/// `string`, failing closed when the value carries entities.
///
/// # Errors
///
/// Returns [`HookError::Unrepresentable`] when `value` has any text entity.
pub fn text_encode(value: &TextWithEntities) -> Result<String> {
    if !value.entities.is_empty() {
        return Err(HookError::Unrepresentable(
            "historical bot verification text entities",
        ));
    }
    Ok(value.text.clone())
}

/// Profile to canonical: the historical plain `string` becomes a
/// [`TextWithEntities`] with no entities.
///
/// # Errors
///
/// Currently infallible; the signature is fallible so field conversions are
/// uniform.
pub fn text_decode(value: &str) -> Result<TextWithEntities> {
    Ok(TextWithEntities {
        text: value.to_owned(),
        entities: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use rtd_tg::{MessageEntity, TextWithEntities};

    use super::{text_decode, text_encode};
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
        assert_eq!(text_encode(&value).unwrap(), "verified");
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
            text_encode(&value).unwrap_err(),
            HookError::Unrepresentable("historical bot verification text entities")
        );
    }

    #[test]
    fn decode_builds_text_without_entities() {
        assert_eq!(text_decode("hello").unwrap(), plain("hello"));
    }
}
