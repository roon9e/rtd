//! `old-only` conversion hooks for the legacy keyboard-button constructors.
//!
//! Layers before 230 encoded an inline-keyboard button's kind as one of many
//! constructor variants (`keyboardButtonRequestPhone`, `keyboardButtonUrl`, …);
//! canonical 232 collapsed them into a single `keyboardButton` carrying a
//! `ButtonType`. These hooks map the legacy constructors to and from the
//! canonical value. A constructor that does not match the canonical value's
//! type fails closed.
//!
//! Decode builds the canonical value from the legacy fields; encode extracts
//! the legacy fields and fails when the canonical type is not the hook's.

use rtd_tg::{
    ButtonType, InlineButtonType, InlineQueryPeerType, InputUser, KeyboardButton,
    KeyboardButtonStyle, KeyboardInlineButton, RequestPeerType,
};

use crate::error::{HookError, Result};

/// The style and text shared by every legacy button shape.
type StyleAndText = (Option<KeyboardButtonStyle>, String);

fn keyboard_button(
    style: Option<KeyboardButtonStyle>,
    text: String,
    ty: ButtonType,
) -> KeyboardButton {
    KeyboardButton {
        // The canonical flags word is derived from presence, matching the
        // canonical codec so a decoded value compares equal to a built one.
        flags: if style.is_some() { 1 << 10 } else { 0 },
        style: style.map(Box::new),
        text,
        type_: Box::new(ty),
    }
}

fn keyboard_inline_button(
    style: Option<KeyboardButtonStyle>,
    text: String,
    ty: InlineButtonType,
) -> KeyboardInlineButton {
    KeyboardInlineButton {
        flags: if style.is_some() { 1 << 10 } else { 0 },
        style: style.map(Box::new),
        text,
        type_: Box::new(ty),
    }
}

fn button_style_text(value: &KeyboardButton, _what: &'static str) -> Result<StyleAndText> {
    Ok((value.style.as_deref().cloned(), value.text.clone()))
}

fn inline_style_text(value: &KeyboardInlineButton) -> Result<StyleAndText> {
    Ok((value.style.as_deref().cloned(), value.text.clone()))
}

/// The fields of the legacy `inputKeyboardButtonRequestPeer`. The three leading
/// presence bits are packed into one tuple argument so the hook stays within the
/// argument limit; the generator packs them the same way (via
/// `legacy_param_tuple_prefix`).
type InputRequestPeerFields = (
    (bool, bool, bool),
    Option<KeyboardButtonStyle>,
    String,
    i32,
    RequestPeerType,
    i32,
);

/// The fields of the legacy `keyboardButtonSwitchInline`.
type SwitchInlineFields = (
    bool,
    Option<KeyboardButtonStyle>,
    String,
    String,
    Option<Vec<InlineQueryPeerType>>,
);

/// The fields of the legacy `keyboardButtonUrlAuth`.
type UrlAuthFields = (
    Option<KeyboardButtonStyle>,
    String,
    Option<String>,
    String,
    i32,
);

/// The fields of the legacy `inputKeyboardButtonUrlAuth`.
type InputUrlAuthFields = (
    bool,
    Option<KeyboardButtonStyle>,
    String,
    Option<String>,
    String,
    InputUser,
);

/// Canonical to profile for `keyboardButtonRequestPhone`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_button_request_phone_style_text(value: &KeyboardButton) -> Result<StyleAndText> {
    if !matches!(*value.type_, ButtonType::ButtonTypeRequestPhone) {
        return Err(HookError::WrongVariant("keyboardButtonRequestPhone"));
    }
    button_style_text(value, "keyboardButtonRequestPhone")
}

/// Profile to canonical for `keyboardButtonRequestPhone`.
#[must_use]
pub fn keyboard_button_request_phone(
    style: Option<KeyboardButtonStyle>,
    text: String,
) -> KeyboardButton {
    keyboard_button(style, text, ButtonType::ButtonTypeRequestPhone)
}

/// Canonical to profile for `keyboardButtonRequestGeoLocation`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_button_request_geo_location_style_text(
    value: &KeyboardButton,
) -> Result<StyleAndText> {
    if !matches!(*value.type_, ButtonType::ButtonTypeRequestGeoLocation) {
        return Err(HookError::WrongVariant("keyboardButtonRequestGeoLocation"));
    }
    button_style_text(value, "keyboardButtonRequestGeoLocation")
}

/// Profile to canonical for `keyboardButtonRequestGeoLocation`.
#[must_use]
pub fn keyboard_button_request_geo_location(
    style: Option<KeyboardButtonStyle>,
    text: String,
) -> KeyboardButton {
    keyboard_button(style, text, ButtonType::ButtonTypeRequestGeoLocation)
}

/// Canonical to profile for `keyboardButtonRequestPoll`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_button_request_poll_fields(
    value: &KeyboardButton,
) -> Result<(Option<KeyboardButtonStyle>, Option<bool>, String)> {
    let ButtonType::ButtonTypeRequestPoll { quiz, .. } = &*value.type_ else {
        return Err(HookError::WrongVariant("keyboardButtonRequestPoll"));
    };
    let (style, text) = button_style_text(value, "keyboardButtonRequestPoll")?;
    Ok((style, *quiz, text))
}

/// Profile to canonical for `keyboardButtonRequestPoll`.
#[must_use]
pub fn keyboard_button_request_poll(
    style: Option<KeyboardButtonStyle>,
    quiz: Option<bool>,
    text: String,
) -> KeyboardButton {
    keyboard_button(
        style,
        text,
        ButtonType::ButtonTypeRequestPoll {
            flags: if quiz.is_some() { 1 } else { 0 },
            quiz,
        },
    )
}

/// Canonical to profile for `keyboardButtonRequestPeer`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_button_request_peer_fields(
    value: &KeyboardButton,
) -> Result<(
    Option<KeyboardButtonStyle>,
    String,
    i32,
    RequestPeerType,
    i32,
)> {
    let ButtonType::ButtonTypeRequestPeer {
        button_id,
        peer_type,
        max_quantity,
        ..
    } = &*value.type_
    else {
        return Err(HookError::WrongVariant("keyboardButtonRequestPeer"));
    };
    let (style, text) = button_style_text(value, "keyboardButtonRequestPeer")?;
    Ok((
        style,
        text,
        *button_id,
        (**peer_type).clone(),
        *max_quantity,
    ))
}

/// Profile to canonical for `keyboardButtonRequestPeer`.
#[must_use]
pub fn keyboard_button_request_peer(
    style: Option<KeyboardButtonStyle>,
    text: String,
    button_id: i32,
    peer_type: RequestPeerType,
    max_quantity: i32,
) -> KeyboardButton {
    keyboard_button(
        style,
        text,
        ButtonType::ButtonTypeRequestPeer {
            flags: 0,
            button_id,
            peer_type: Box::new(peer_type),
            max_quantity,
        },
    )
}

/// Canonical to profile for `inputKeyboardButtonRequestPeer` (the legacy
/// input-only request-peer shape).
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn input_keyboard_button_request_peer_fields(
    value: &KeyboardButton,
) -> Result<InputRequestPeerFields> {
    let ButtonType::InputButtonTypeRequestPeer {
        name_requested,
        username_requested,
        photo_requested,
        button_id,
        peer_type,
        max_quantity,
        ..
    } = &*value.type_
    else {
        return Err(HookError::WrongVariant("inputKeyboardButtonRequestPeer"));
    };
    let (style, text) = button_style_text(value, "inputKeyboardButtonRequestPeer")?;
    Ok((
        (*name_requested, *username_requested, *photo_requested),
        style,
        text,
        *button_id,
        (**peer_type).clone(),
        *max_quantity,
    ))
}

/// Profile to canonical for `inputKeyboardButtonRequestPeer`.
#[must_use]
pub fn input_keyboard_button_request_peer(
    requested: (bool, bool, bool),
    style: Option<KeyboardButtonStyle>,
    text: String,
    button_id: i32,
    peer_type: RequestPeerType,
    max_quantity: i32,
) -> KeyboardButton {
    let (name_requested, username_requested, photo_requested) = requested;
    keyboard_button(
        style,
        text,
        ButtonType::InputButtonTypeRequestPeer {
            flags: u32::from(name_requested)
                | (u32::from(username_requested) << 1)
                | (u32::from(photo_requested) << 2),
            name_requested,
            username_requested,
            photo_requested,
            button_id,
            peer_type: Box::new(peer_type),
            max_quantity,
        },
    )
}

/// Canonical to profile for `keyboardButtonSimpleWebView`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_button_simple_web_view_fields(
    value: &KeyboardButton,
) -> Result<(Option<KeyboardButtonStyle>, String, String)> {
    let ButtonType::ButtonTypeSimpleWebView { url } = &*value.type_ else {
        return Err(HookError::WrongVariant("keyboardButtonSimpleWebView"));
    };
    let (style, text) = button_style_text(value, "keyboardButtonSimpleWebView")?;
    Ok((style, text, url.clone()))
}

/// Profile to canonical for `keyboardButtonSimpleWebView`.
#[must_use]
pub fn keyboard_button_simple_web_view(
    style: Option<KeyboardButtonStyle>,
    text: String,
    url: String,
) -> KeyboardButton {
    keyboard_button(style, text, ButtonType::ButtonTypeSimpleWebView { url })
}

fn inline_fields(
    value: &KeyboardInlineButton,
    what: &'static str,
) -> Result<(Option<KeyboardButtonStyle>, String)> {
    let _ = what;
    inline_style_text(value)
}

/// Canonical to profile for `keyboardButtonUrl` / `keyboardInlineButtonUrl`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_url_fields(
    value: &KeyboardInlineButton,
) -> Result<(Option<KeyboardButtonStyle>, String, String)> {
    let InlineButtonType::InlineButtonTypeUrl { url } = &*value.type_ else {
        return Err(HookError::WrongVariant("keyboardButtonUrl"));
    };
    let (style, text) = inline_fields(value, "keyboardButtonUrl")?;
    Ok((style, text, url.clone()))
}

/// Profile to canonical for `keyboardButtonUrl`.
#[must_use]
pub fn keyboard_inline_button_url(
    style: Option<KeyboardButtonStyle>,
    text: String,
    url: String,
) -> KeyboardInlineButton {
    keyboard_inline_button(style, text, InlineButtonType::InlineButtonTypeUrl { url })
}

/// Canonical to profile for `keyboardButtonCallback`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_callback_fields(
    value: &KeyboardInlineButton,
) -> Result<(bool, Option<KeyboardButtonStyle>, String, Vec<u8>)> {
    let InlineButtonType::InlineButtonTypeCallback {
        requires_password,
        data,
        ..
    } = &*value.type_
    else {
        return Err(HookError::WrongVariant("keyboardButtonCallback"));
    };
    let (style, text) = inline_fields(value, "keyboardButtonCallback")?;
    Ok((*requires_password, style, text, data.clone()))
}

/// Profile to canonical for `keyboardButtonCallback`.
#[must_use]
pub fn keyboard_inline_button_callback(
    requires_password: bool,
    style: Option<KeyboardButtonStyle>,
    text: String,
    data: Vec<u8>,
) -> KeyboardInlineButton {
    keyboard_inline_button(
        style,
        text,
        InlineButtonType::InlineButtonTypeCallback {
            flags: u32::from(requires_password),
            requires_password,
            data,
        },
    )
}

/// The switch-inline fields (same-peer, query, optional peer types).
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_switch_inline_fields(
    value: &KeyboardInlineButton,
) -> Result<SwitchInlineFields> {
    let InlineButtonType::InlineButtonTypeSwitchInline {
        same_peer,
        query,
        peer_types,
        ..
    } = &*value.type_
    else {
        return Err(HookError::WrongVariant("keyboardButtonSwitchInline"));
    };
    let (style, text) = inline_fields(value, "keyboardButtonSwitchInline")?;
    Ok((*same_peer, style, text, query.clone(), peer_types.clone()))
}

/// Profile to canonical for `keyboardButtonSwitchInline`.
#[must_use]
pub fn keyboard_inline_button_switch_inline(
    same_peer: bool,
    style: Option<KeyboardButtonStyle>,
    text: String,
    query: String,
    peer_types: Option<Vec<InlineQueryPeerType>>,
) -> KeyboardInlineButton {
    keyboard_inline_button(
        style,
        text,
        InlineButtonType::InlineButtonTypeSwitchInline {
            flags: u32::from(same_peer) | (u32::from(peer_types.is_some()) << 1),
            same_peer,
            query,
            peer_types,
        },
    )
}

/// Canonical to profile for `keyboardButtonGame`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_game_fields(
    value: &KeyboardInlineButton,
) -> Result<(Option<KeyboardButtonStyle>, String)> {
    if !matches!(*value.type_, InlineButtonType::InlineButtonTypeGame) {
        return Err(HookError::WrongVariant("keyboardButtonGame"));
    }
    inline_fields(value, "keyboardButtonGame")
}

/// Profile to canonical for `keyboardButtonGame`.
#[must_use]
pub fn keyboard_inline_button_game(
    style: Option<KeyboardButtonStyle>,
    text: String,
) -> KeyboardInlineButton {
    keyboard_inline_button(style, text, InlineButtonType::InlineButtonTypeGame)
}

/// Canonical to profile for `keyboardButtonBuy`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_buy_fields(
    value: &KeyboardInlineButton,
) -> Result<(Option<KeyboardButtonStyle>, String)> {
    if !matches!(*value.type_, InlineButtonType::InlineButtonTypeBuy) {
        return Err(HookError::WrongVariant("keyboardButtonBuy"));
    }
    inline_fields(value, "keyboardButtonBuy")
}

/// Profile to canonical for `keyboardButtonBuy`.
#[must_use]
pub fn keyboard_inline_button_buy(
    style: Option<KeyboardButtonStyle>,
    text: String,
) -> KeyboardInlineButton {
    keyboard_inline_button(style, text, InlineButtonType::InlineButtonTypeBuy)
}

/// Canonical to profile for `keyboardButtonUrlAuth`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_url_auth_fields(
    value: &KeyboardInlineButton,
) -> Result<UrlAuthFields> {
    let InlineButtonType::InlineButtonTypeUrlAuth {
        fwd_text,
        url,
        button_id,
        ..
    } = &*value.type_
    else {
        return Err(HookError::WrongVariant("keyboardButtonUrlAuth"));
    };
    let (style, text) = inline_fields(value, "keyboardButtonUrlAuth")?;
    Ok((style, text, fwd_text.clone(), url.clone(), *button_id))
}

/// Profile to canonical for `keyboardButtonUrlAuth`.
#[must_use]
pub fn keyboard_inline_button_url_auth(
    style: Option<KeyboardButtonStyle>,
    text: String,
    fwd_text: Option<String>,
    url: String,
    button_id: i32,
) -> KeyboardInlineButton {
    keyboard_inline_button(
        style,
        text,
        InlineButtonType::InlineButtonTypeUrlAuth {
            flags: u32::from(fwd_text.is_some()),
            fwd_text,
            url,
            button_id,
        },
    )
}

/// Canonical to profile for `inputKeyboardButtonUrlAuth`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn input_keyboard_inline_button_url_auth_fields(
    value: &KeyboardInlineButton,
) -> Result<InputUrlAuthFields> {
    let InlineButtonType::InputInlineButtonTypeUrlAuth {
        request_write_access,
        fwd_text,
        url,
        bot,
        ..
    } = &*value.type_
    else {
        return Err(HookError::WrongVariant("inputKeyboardButtonUrlAuth"));
    };
    let bot = bot
        .as_deref()
        .cloned()
        .ok_or(HookError::Missing("inputKeyboardButtonUrlAuth.bot"))?;
    let (style, text) = inline_fields(value, "inputKeyboardButtonUrlAuth")?;
    Ok((
        *request_write_access,
        style,
        text,
        fwd_text.clone(),
        url.clone(),
        bot,
    ))
}

/// Profile to canonical for `inputKeyboardButtonUrlAuth`.
#[must_use]
pub fn input_keyboard_inline_button_url_auth(
    request_write_access: bool,
    style: Option<KeyboardButtonStyle>,
    text: String,
    fwd_text: Option<String>,
    url: String,
    bot: InputUser,
) -> KeyboardInlineButton {
    keyboard_inline_button(
        style,
        text,
        InlineButtonType::InputInlineButtonTypeUrlAuth {
            flags: u32::from(request_write_access)
                | (u32::from(fwd_text.is_some()) << 1)
                | (1 << 2),
            request_write_access,
            fwd_text,
            url,
            bot: Some(Box::new(bot)),
        },
    )
}

/// Canonical to profile for `keyboardButtonUserProfile`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_user_profile_fields(
    value: &KeyboardInlineButton,
) -> Result<(Option<KeyboardButtonStyle>, String, i64)> {
    let InlineButtonType::InlineButtonTypeUserProfile { user_id } = &*value.type_ else {
        return Err(HookError::WrongVariant("keyboardButtonUserProfile"));
    };
    let (style, text) = inline_fields(value, "keyboardButtonUserProfile")?;
    Ok((style, text, *user_id))
}

/// Profile to canonical for `keyboardButtonUserProfile`.
#[must_use]
pub fn keyboard_inline_button_user_profile(
    style: Option<KeyboardButtonStyle>,
    text: String,
    user_id: i64,
) -> KeyboardInlineButton {
    keyboard_inline_button(
        style,
        text,
        InlineButtonType::InlineButtonTypeUserProfile { user_id },
    )
}

/// Canonical to profile for `inputKeyboardButtonUserProfile`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn input_keyboard_inline_button_user_profile_fields(
    value: &KeyboardInlineButton,
) -> Result<(Option<KeyboardButtonStyle>, String, InputUser)> {
    let InlineButtonType::InputInlineButtonTypeUserProfile { user_id } = &*value.type_ else {
        return Err(HookError::WrongVariant("inputKeyboardButtonUserProfile"));
    };
    let (style, text) = inline_fields(value, "inputKeyboardButtonUserProfile")?;
    Ok((style, text, (**user_id).clone()))
}

/// Profile to canonical for `inputKeyboardButtonUserProfile`.
#[must_use]
pub fn input_keyboard_inline_button_user_profile(
    style: Option<KeyboardButtonStyle>,
    text: String,
    user_id: InputUser,
) -> KeyboardInlineButton {
    keyboard_inline_button(
        style,
        text,
        InlineButtonType::InputInlineButtonTypeUserProfile {
            user_id: Box::new(user_id),
        },
    )
}

/// Canonical to profile for `keyboardButtonWebView`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_web_view_fields(
    value: &KeyboardInlineButton,
) -> Result<(Option<KeyboardButtonStyle>, String, String)> {
    let InlineButtonType::InlineButtonTypeWebView { url } = &*value.type_ else {
        return Err(HookError::WrongVariant("keyboardButtonWebView"));
    };
    let (style, text) = inline_fields(value, "keyboardButtonWebView")?;
    Ok((style, text, url.clone()))
}

/// Profile to canonical for `keyboardButtonWebView`.
#[must_use]
pub fn keyboard_inline_button_web_view(
    style: Option<KeyboardButtonStyle>,
    text: String,
    url: String,
) -> KeyboardInlineButton {
    keyboard_inline_button(
        style,
        text,
        InlineButtonType::InlineButtonTypeWebView { url },
    )
}

/// Canonical to profile for `keyboardButtonCopy`.
///
/// # Errors
///
/// Returns [`HookError::WrongVariant`] when the button is a different type.
pub fn keyboard_inline_button_copy_fields(
    value: &KeyboardInlineButton,
) -> Result<(Option<KeyboardButtonStyle>, String, String)> {
    let InlineButtonType::InlineButtonTypeCopy { copy_text } = &*value.type_ else {
        return Err(HookError::WrongVariant("keyboardButtonCopy"));
    };
    let (style, text) = inline_fields(value, "keyboardButtonCopy")?;
    Ok((style, text, copy_text.clone()))
}

/// Profile to canonical for `keyboardButtonCopy`.
#[must_use]
pub fn keyboard_inline_button_copy(
    style: Option<KeyboardButtonStyle>,
    text: String,
    copy_text: String,
) -> KeyboardInlineButton {
    keyboard_inline_button(
        style,
        text,
        InlineButtonType::InlineButtonTypeCopy { copy_text },
    )
}

#[cfg(test)]
mod tests {
    use rtd_tg::{ButtonType, InlineButtonType, KeyboardButton, KeyboardInlineButton};

    use super::{
        keyboard_button_request_phone, keyboard_button_request_phone_style_text,
        keyboard_button_simple_web_view, keyboard_button_simple_web_view_fields,
        keyboard_inline_button_callback, keyboard_inline_button_callback_fields,
        keyboard_inline_button_copy, keyboard_inline_button_copy_fields,
        keyboard_inline_button_url, keyboard_inline_button_url_fields,
    };
    use crate::error::HookError;

    #[test]
    fn request_phone_round_trips() {
        let value = keyboard_button_request_phone(None, "call".to_owned());
        assert!(matches!(*value.type_, ButtonType::ButtonTypeRequestPhone));
        let (style, text) = keyboard_button_request_phone_style_text(&value).unwrap();
        assert!(style.is_none());
        assert_eq!(text, "call");
    }

    #[test]
    fn wrong_variant_fails_closed() {
        let value = keyboard_button_simple_web_view(None, "x".to_owned(), "u".to_owned());
        assert!(matches!(
            keyboard_button_request_phone_style_text(&value).unwrap_err(),
            HookError::WrongVariant(_)
        ));
        let (_, _, url) = keyboard_button_simple_web_view_fields(&value).unwrap();
        assert_eq!(url, "u");
    }

    #[test]
    fn inline_url_round_trips() {
        let value = keyboard_inline_button_url(None, "open".to_owned(), "https://x".to_owned());
        assert!(matches!(
            *value.type_,
            InlineButtonType::InlineButtonTypeUrl { .. }
        ));
        let (_, text, url) = keyboard_inline_button_url_fields(&value).unwrap();
        assert_eq!(text, "open");
        assert_eq!(url, "https://x");
    }

    #[test]
    fn inline_callback_round_trips() {
        let value = keyboard_inline_button_callback(true, None, "go".to_owned(), vec![1, 2]);
        let (requires_password, _, _, data) =
            keyboard_inline_button_callback_fields(&value).unwrap();
        assert!(requires_password);
        assert_eq!(data, vec![1, 2]);
    }

    #[test]
    fn inline_copy_round_trips() {
        let value = keyboard_inline_button_copy(None, "copy".to_owned(), "text".to_owned());
        let (_, _, copy_text) = keyboard_inline_button_copy_fields(&value).unwrap();
        assert_eq!(copy_text, "text");
    }

    #[test]
    fn an_unbuilt_button_is_a_wrong_variant() {
        let value = KeyboardButton {
            flags: 0,
            style: None,
            text: "x".to_owned(),
            type_: Box::new(ButtonType::ButtonTypeDefault),
        };
        assert!(matches!(
            keyboard_button_request_phone_style_text(&value).unwrap_err(),
            HookError::WrongVariant(_)
        ));
        let inline = KeyboardInlineButton {
            flags: 0,
            style: None,
            text: "x".to_owned(),
            type_: Box::new(InlineButtonType::InlineButtonTypeGame),
        };
        assert!(matches!(
            keyboard_inline_button_url_fields(&inline).unwrap_err(),
            HookError::WrongVariant(_)
        ));
    }
}
