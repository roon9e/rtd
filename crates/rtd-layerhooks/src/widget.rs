//! Family-level media projection hooks.
//!
//! Unlike the field-shaped projections, a *family* projection maps a whole
//! canonical class value to a different canonical constructor the older profile
//! can represent, because dropping the member would change the surrounding
//! message's meaning.

use rtd_tg::MessageMedia;

use crate::error::Result;

/// Canonical-to-profile projection for `messageMedia`'s `messageMediaWidget`.
///
/// A Layer 233 widget has no wire identity before Layer 233; dropping the media
/// would reach an older client as an empty text message. Before Layer 233 the
/// member is projected to `messageMediaUnsupported`, so the older client renders
/// an explicit unsupported-media placeholder instead.
///
/// # Errors
///
/// Currently infallible; the signature is fallible for uniformity.
pub fn project_legacy_widget_media(layer: i32, value: &MessageMedia) -> Result<MessageMedia> {
    match value {
        MessageMedia::MessageMediaWidget { .. } if layer < 233 => {
            Ok(MessageMedia::MessageMediaUnsupported)
        }
        _ => Ok(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    use rtd_tg::MessageMedia;

    use super::project_legacy_widget_media;

    fn widget() -> MessageMedia {
        MessageMedia::MessageMediaWidget {
            flags: 0,
            is_copy: false,
            can_query: false,
            background_query: false,
            width: 0,
            height: 0,
            payload: Vec::new(),
            public_data: None,
            public_data_hash: None,
            chat_data: None,
            widget_id: None,
        }
    }

    #[test]
    fn a_widget_projects_to_unsupported_before_233() {
        assert_eq!(
            project_legacy_widget_media(232, &widget()).unwrap(),
            MessageMedia::MessageMediaUnsupported
        );
        assert_eq!(
            project_legacy_widget_media(225, &widget()).unwrap(),
            MessageMedia::MessageMediaUnsupported
        );
    }

    #[test]
    fn a_widget_survives_at_233() {
        assert_eq!(
            project_legacy_widget_media(233, &widget()).unwrap(),
            widget()
        );
    }

    #[test]
    fn another_member_is_unchanged() {
        assert_eq!(
            project_legacy_widget_media(225, &MessageMedia::MessageMediaEmpty).unwrap(),
            MessageMedia::MessageMediaEmpty
        );
    }
}
