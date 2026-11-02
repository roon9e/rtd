//! Shared signature registry for the per-layer conversion hooks.
//!
//! The reviewed policy in `schema/layers/policy.json` resolves a non-mechanical
//! difference to a named hook. The conversion logic lives in `rtd-layerhooks`;
//! this crate is the single source of truth that both the generator and the
//! hooks crate consume, so a hook cannot be referenced without a signature and a
//! signature cannot go stale.
//!
//! Only the **present-in-both** adapter hooks are registered here. Hooks whose
//! family exists only in an older layer (`old-only` / `ProfileOnly`) are deferred
//! to the `add-legacy-layer-constructors` change and are absent by design.
#![warn(missing_docs)]

/// The direction a hook converts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Canonical layer to an older profile.
    CanonicalToProfile,
    /// An older profile to the canonical layer.
    ProfileToCanonical,
    /// Both directions.
    Both,
}

impl Direction {
    /// Whether this direction serves an obligation with `direction` (one of
    /// `canonical-to-profile`, `profile-to-canonical` or `both`).
    #[must_use]
    pub fn covers(self, direction: &str) -> bool {
        match self {
            Direction::Both => matches!(direction, "canonical-to-profile" | "profile-to-canonical"),
            Direction::CanonicalToProfile => direction == "canonical-to-profile",
            Direction::ProfileToCanonical => direction == "profile-to-canonical",
        }
    }
}

/// Whether a registered hook is implemented in `rtd-layerhooks`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Implemented; generation may reference it.
    Implemented,
    /// Registered but not yet implemented; generation fails closed on it.
    Pending,
    /// Present-in-both but deferred because it transitively needs a `ProfileOnly`
    /// constructor; generation fails closed at runtime for the affected values.
    Deferred,
    /// Implemented by the generator as a nested codec rather than as a
    /// hand-written function in `rtd-layerhooks`; generation may reference it.
    Generated,
}

/// One hook's signature, keyed by the name used in the reviewed policy.
#[derive(Debug, Clone, Copy)]
pub struct HookSpec {
    /// The hook name as written in `policy.json`.
    pub name: &'static str,
    /// The directions this hook converts.
    pub direction: Direction,
    /// Whether the hook is implemented.
    pub status: Status,
    /// The reference signature (from `td/tlprofile/adapters.go`), for review.
    pub signature: &'static str,
}

/// Every present-in-both adapter hook, ordered by name.
pub const HOOKS: &[HookSpec] = &[
    HookSpec {
        name: "layerAdaptBotGuestChatResult",
        direction: Direction::CanonicalToProfile,
        status: Status::Implemented,
        signature: "layerAdaptBotGuestChatResult(_ LayerProfile, messageID InputBotInlineMessageID) (Bool, error)",
    },
    HookSpec {
        name: "layerAdaptChatInviteJoinResultToUpdates",
        direction: Direction::CanonicalToProfile,
        status: Status::Implemented,
        signature: "layerAdaptChatInviteJoinResultToUpdates(_ LayerProfile, result MessagesChatInviteJoinResult) (Updates, error)",
    },
    HookSpec {
        name: "layerAdaptLegacyInlineKeyboardRows",
        direction: Direction::Both,
        // The rows codec is generated from the profile `KeyboardButtonRow`
        // layout and dispatches each legacy button to its registered `old-only`
        // hook; see `legacy_rows`.
        status: Status::Generated,
        signature: "layerAdaptLegacyInlineKeyboardRows(_ LayerProfile, rows Vector<KeyboardButtonRow>/Vector<KeyboardInlineButtonRow>) (Vector<...>, error)",
    },
    HookSpec {
        name: "layerAdaptUpdatesToChatInviteJoinResult",
        direction: Direction::ProfileToCanonical,
        status: Status::Implemented,
        signature: "layerAdaptUpdatesToChatInviteJoinResult(_ LayerProfile, updates Updates) (MessagesChatInviteJoinResult, error)",
    },
    HookSpec {
        name: "layerAdaptWebBrowserSettingsExceptionResult",
        direction: Direction::CanonicalToProfile,
        status: Status::Implemented,
        signature: "layerAdaptWebBrowserSettingsExceptionResult(_ LayerProfile, updates Updates) (bool, error)",
    },
    HookSpec {
        name: "layerBotVerificationText",
        direction: Direction::Both,
        status: Status::Implemented,
        signature: "layerBotVerificationText{Encode,Decode}(_ LayerProfile, _ *BotVerification, value TextWithEntities|string) (string|TextWithEntities, error)",
    },
    HookSpec {
        name: "layerBotVerifierSettingsText",
        direction: Direction::Both,
        status: Status::Implemented,
        signature: "layerBotVerifierSettingsText{Encode,Decode}(_ LayerProfile, _ *BotVerifierSettings, present bool, value TextWithEntities|string) (string|TextWithEntities, bool, error)",
    },
    HookSpec {
        name: "layerBotsSetCustomVerificationText",
        direction: Direction::Both,
        status: Status::Implemented,
        signature: "layerBotsSetCustomVerificationText{Encode,Decode}(_ LayerProfile, _ *BotsSetCustomVerification, present bool, value TextWithEntities|string) (string|TextWithEntities, bool, error)",
    },
    HookSpec {
        name: "layerCaptureChatInviteJoinQueryIDFromWebView",
        direction: Direction::ProfileToCanonical,
        status: Status::Implemented,
        signature: "layerCaptureChatInviteJoinQueryIDFromWebView(_ LayerProfile, value MessagesChatInviteJoinResultWebView, webview WebViewResult) error",
    },
    HookSpec {
        name: "layerDefaultLegacyKeyboardButtonType",
        direction: Direction::ProfileToCanonical,
        status: Status::Implemented,
        signature: "layerDefaultLegacyKeyboardButtonTypeDecode(_ LayerProfile, value *KeyboardButton) (ButtonType, error)",
    },
    HookSpec {
        name: "layerProjectEphemeralBotCommand",
        direction: Direction::CanonicalToProfile,
        status: Status::Implemented,
        signature: "project_ephemeral_bot_command(value *BotCommand) (*BotCommand, error); None drops the value",
    },
    HookSpec {
        name: "layerProjectLegacyDefaultKeyboardButton",
        direction: Direction::CanonicalToProfile,
        status: Status::Implemented,
        signature: "project_default_button_type(value *KeyboardButton) (*KeyboardButton, error); None drops the value",
    },
    HookSpec {
        name: "layerRequireCapturedChatInviteJoinQueryID",
        direction: Direction::ProfileToCanonical,
        status: Status::Implemented,
        signature: "layerRequireCapturedChatInviteJoinQueryIDDecode(_ LayerProfile, value MessagesChatInviteJoinResultWebView) (int64, error)",
    },
    HookSpec {
        name: "layerRequireLegacyEphemeralDeletePeer",
        direction: Direction::CanonicalToProfile,
        status: Status::Implemented,
        signature: "layerRequireLegacyEphemeralDeletePeerEncode(_ LayerProfile, _ *EphemeralDeleteMessage, present bool, peer InputPeer) (InputPeer, error)",
    },
    HookSpec {
        name: "layerRequireLegacyEphemeralMessagePeer",
        direction: Direction::CanonicalToProfile,
        status: Status::Implemented,
        signature: "layerRequireLegacyEphemeralMessagePeerEncode(_ LayerProfile, _ *EphemeralMessage, present bool, peer Peer) (Peer, error)",
    },
    HookSpec {
        name: "layerRequireLegacyEphemeralSendPeer",
        direction: Direction::CanonicalToProfile,
        status: Status::Implemented,
        signature: "layerRequireLegacyEphemeralSendPeerEncode(_ LayerProfile, _ *EphemeralSendMessage, present bool, peer InputPeer) (InputPeer, error)",
    },
];

/// Looks up a hook signature by name.
#[must_use]
pub fn lookup(name: &str) -> Option<&'static HookSpec> {
    HOOKS.iter().find(|spec| spec.name == name)
}

/// The canonical value a legacy button constructor maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyCanonical {
    /// The reply-keyboard `KeyboardButton`.
    KeyboardButton,
    /// The inline `KeyboardInlineButton`.
    KeyboardInlineButton,
}

/// An `old-only` hook: a legacy keyboard-button constructor that exists only in
/// an older layer, mapped to and from the canonical value it became.
#[derive(Debug, Clone, Copy)]
pub struct OldOnlyHook {
    /// The hook name as written in `policy.json`.
    pub name: &'static str,
    /// The legacy constructor wire id in the older layer.
    pub wire_id: u32,
    /// The canonical value the constructor maps to.
    pub canonical: LegacyCanonical,
    /// The canonical enum variant the constructor became, within the enum named
    /// by [`Self::canonical`] (`ButtonType` or `InlineButtonType`).
    pub variant: &'static str,
    /// The profile-to-canonical builder, taking the legacy fields in order.
    pub decode: &'static str,
    /// The canonical-to-profile field extractor.
    pub encode: &'static str,
}

/// Every `old-only` hook, ordered by wire id.
///
/// `inputPasskeyCredentialFirebasePNV` is the one `ProfileOnly` constructor with
/// no legacy hook, so it stays fail-closed.
pub const LEGACY_HOOKS: &[OldOnlyHook] = &[
    OldOnlyHook {
        name: "layerAdaptLegacyInputKeyboardButtonRequestPeer",
        wire_id: 0x02b7_8156,
        canonical: LegacyCanonical::KeyboardButton,
        variant: "InputButtonTypeRequestPeer",
        decode: "rtd_layerhooks::legacy::input_keyboard_button_request_peer",
        encode: "rtd_layerhooks::legacy::input_keyboard_button_request_peer_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardButtonBuy",
        wire_id: 0x3fa5_3905,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeBuy",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_buy",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_buy_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardButtonRequestPhone",
        wire_id: 0x417e_fd8f,
        canonical: LegacyCanonical::KeyboardButton,
        variant: "ButtonTypeRequestPhone",
        decode: "rtd_layerhooks::legacy::keyboard_button_request_phone",
        encode: "rtd_layerhooks::legacy::keyboard_button_request_phone_style_text",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardButtonRequestPeer",
        wire_id: 0x5b0f_15f5,
        canonical: LegacyCanonical::KeyboardButton,
        variant: "ButtonTypeRequestPeer",
        decode: "rtd_layerhooks::legacy::keyboard_button_request_peer",
        encode: "rtd_layerhooks::legacy::keyboard_button_request_peer_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyInputKeyboardInlineButtonURLAuth",
        wire_id: 0x6801_3e72,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InputInlineButtonTypeUrlAuth",
        decode: "rtd_layerhooks::legacy::input_keyboard_inline_button_url_auth",
        encode: "rtd_layerhooks::legacy::input_keyboard_inline_button_url_auth_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardButtonRequestPoll",
        wire_id: 0x7a11_d782,
        canonical: LegacyCanonical::KeyboardButton,
        variant: "ButtonTypeRequestPoll",
        decode: "rtd_layerhooks::legacy::keyboard_button_request_poll",
        encode: "rtd_layerhooks::legacy::keyboard_button_request_poll_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyInputKeyboardInlineButtonUserProfile",
        wire_id: 0x7d5e_07c7,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InputInlineButtonTypeUserProfile",
        decode: "rtd_layerhooks::legacy::input_keyboard_inline_button_user_profile",
        encode: "rtd_layerhooks::legacy::input_keyboard_inline_button_user_profile_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardInlineButtonGame",
        wire_id: 0x89c5_90f9,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeGame",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_game",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_game_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardInlineButtonSwitchInline",
        wire_id: 0x9913_99fc,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeSwitchInline",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_switch_inline",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_switch_inline_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardButtonRequestGeoLocation",
        wire_id: 0xaa40_f94d,
        canonical: LegacyCanonical::KeyboardButton,
        variant: "ButtonTypeRequestGeoLocation",
        decode: "rtd_layerhooks::legacy::keyboard_button_request_geo_location",
        encode: "rtd_layerhooks::legacy::keyboard_button_request_geo_location_style_text",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardInlineButtonCopy",
        wire_id: 0xbcc4_af10,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeCopy",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_copy",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_copy_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardInlineButtonUserProfile",
        wire_id: 0xc0fd_5d09,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeUserProfile",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_user_profile",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_user_profile_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardInlineButtonURL",
        wire_id: 0xd80c_25ec,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeUrl",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_url",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_url_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardButtonSimpleWebView",
        wire_id: 0xe15c_4370,
        canonical: LegacyCanonical::KeyboardButton,
        variant: "ButtonTypeSimpleWebView",
        decode: "rtd_layerhooks::legacy::keyboard_button_simple_web_view",
        encode: "rtd_layerhooks::legacy::keyboard_button_simple_web_view_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardInlineButtonCallback",
        wire_id: 0xe62b_c960,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeCallback",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_callback",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_callback_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardInlineButtonWebView",
        wire_id: 0xe846_b1a0,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeWebView",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_web_view",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_web_view_fields",
    },
    OldOnlyHook {
        name: "layerAdaptLegacyKeyboardInlineButtonURLAuth",
        wire_id: 0xf510_06f9,
        canonical: LegacyCanonical::KeyboardInlineButton,
        variant: "InlineButtonTypeUrlAuth",
        decode: "rtd_layerhooks::legacy::keyboard_inline_button_url_auth",
        encode: "rtd_layerhooks::legacy::keyboard_inline_button_url_auth_fields",
    },
];

/// Looks up an `old-only` hook by the legacy constructor wire id.
#[must_use]
pub fn legacy_for_wire(wire_id: u32) -> Option<&'static OldOnlyHook> {
    LEGACY_HOOKS.iter().find(|hook| hook.wire_id == wire_id)
}

/// The shape of a nested legacy keyboard-rows adapter: a class-level hook that
/// maps a whole vector of button rows between the older profile's
/// `KeyboardButtonRow` and the canonical `KeyboardInlineButtonRow`.
#[derive(Debug, Clone, Copy)]
pub struct LegacyRowsShape {
    /// The canonical button kind whose `old-only` constructors appear in the
    /// rows; the generator dispatches every legacy button wire id registered for
    /// this kind through the rows codec.
    pub canonical: LegacyCanonical,
}

/// Marks a hook as a nested legacy-rows adapter, when it is one.
///
/// The row wire id and the per-button layouts come from the profile schema; this
/// only selects which registered `old-only` hooks the rows codec may dispatch.
#[must_use]
pub fn legacy_rows(name: &str) -> Option<LegacyRowsShape> {
    match name {
        "layerAdaptLegacyInlineKeyboardRows" => Some(LegacyRowsShape {
            canonical: LegacyCanonical::KeyboardInlineButton,
        }),
        _ => None,
    }
}

/// The value-field order a legacy hook expects, which must equal the profile
/// constructor's declaration order (flags word excluded). The generator emits
/// fields in this order and the hand-written hook takes them in the same order.
#[must_use]
pub fn legacy_params(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "layerAdaptLegacyInputKeyboardButtonRequestPeer" => &[
            "name_requested",
            "username_requested",
            "photo_requested",
            "style",
            "text",
            "button_id",
            "peer_type",
            "max_quantity",
        ],
        "layerAdaptLegacyKeyboardButtonBuy"
        | "layerAdaptLegacyKeyboardButtonRequestPhone"
        | "layerAdaptLegacyKeyboardButtonRequestGeoLocation"
        | "layerAdaptLegacyKeyboardInlineButtonGame" => &["style", "text"],
        "layerAdaptLegacyKeyboardButtonRequestPeer" => {
            &["style", "text", "button_id", "peer_type", "max_quantity"]
        }
        "layerAdaptLegacyInputKeyboardInlineButtonURLAuth" => &[
            "request_write_access",
            "style",
            "text",
            "fwd_text",
            "url",
            "bot",
        ],
        "layerAdaptLegacyKeyboardButtonRequestPoll" => &["style", "quiz", "text"],
        "layerAdaptLegacyInputKeyboardInlineButtonUserProfile"
        | "layerAdaptLegacyKeyboardInlineButtonUserProfile" => &["style", "text", "user_id"],
        "layerAdaptLegacyKeyboardInlineButtonSwitchInline" => {
            &["same_peer", "style", "text", "query", "peer_types"]
        }
        "layerAdaptLegacyKeyboardInlineButtonCopy" => &["style", "text", "copy_text"],
        "layerAdaptLegacyKeyboardInlineButtonURL"
        | "layerAdaptLegacyKeyboardButtonSimpleWebView"
        | "layerAdaptLegacyKeyboardInlineButtonWebView" => &["style", "text", "url"],
        "layerAdaptLegacyKeyboardInlineButtonCallback" => {
            &["requires_password", "style", "text", "data"]
        }
        "layerAdaptLegacyKeyboardInlineButtonURLAuth" => {
            &["style", "text", "fwd_text", "url", "button_id"]
        }
        _ => return None,
    })
}

/// The concrete functions of a field-shaped hook, which converts one field
/// between its canonical and older-layer types (the surrounding `Option` is
/// handled by the generated call site).
#[derive(Debug, Clone, Copy)]
pub struct FieldConvert {
    /// Canonical field value to the profile field value.
    pub encode: &'static str,
    /// Profile field value to the canonical field value.
    pub decode: &'static str,
}

/// The field-conversion functions for a hook, when it is field-shaped.
///
/// Hooks that are not field-shaped (result, presence, discard, vector or
/// projection hooks) return `None`: their call sites need a different emission
/// shape that is not generated yet, so the layer stays fail-closed.
#[must_use]
pub fn field_convert(name: &str) -> Option<FieldConvert> {
    let convert = match name {
        // A `TextWithEntities` <-> plain `string` projection: the historical
        // shape cannot carry entities, so entities fail closed.
        "layerBotVerificationText"
        | "layerBotVerifierSettingsText"
        | "layerBotsSetCustomVerificationText" => FieldConvert {
            encode: "rtd_layerhooks::text::text_encode",
            decode: "rtd_layerhooks::text::text_decode",
        },
        _ => return None,
    };
    Some(convert)
}

/// The projection function for a hook that drops a canonical value the older
/// layer cannot represent. It takes `&Canonical` and returns `Some(value)` to
/// keep it or `None` to project it away.
///
/// Hooks with any other shape return `None`, so their layer stays fail-closed.
#[must_use]
pub fn field_project(name: &str) -> Option<&'static str> {
    Some(match name {
        "layerProjectEphemeralBotCommand" => {
            "rtd_layerhooks::projection::project_ephemeral_bot_command"
        }
        "layerProjectLegacyDefaultKeyboardButton" => {
            "rtd_layerhooks::keyboard::project_default_button_type"
        }
        _ => return None,
    })
}

/// The supply function for a hook that supplies a mandatory canonical field the
/// profile omits. It takes no arguments and returns the canonical value.
///
/// Hooks with any other shape return `None`, so their layer stays fail-closed.
#[must_use]
pub fn field_supply(name: &str) -> Option<&'static str> {
    Some(match name {
        "layerDefaultLegacyKeyboardButtonType" => {
            "rtd_layerhooks::keyboard::supply_default_button_type"
        }
        _ => return None,
    })
}

/// A capture hook: a mandatory canonical field the profile omits is derived from
/// another profile field (e.g. the chat invite query id captured from `webview`).
#[derive(Debug, Clone, Copy)]
pub struct Capture {
    /// The capture function, taking the source profile field by reference.
    pub function: &'static str,
    /// The canonical field the capture fills.
    pub target: &'static str,
}

/// The capture hook for a name, when it is a capture.
///
/// Hooks with any other shape return `None`, so their layer stays fail-closed.
#[must_use]
pub fn field_capture(name: &str) -> Option<Capture> {
    Some(match name {
        "layerCaptureChatInviteJoinQueryIDFromWebView"
        | "layerRequireCapturedChatInviteJoinQueryID" => Capture {
            function: "rtd_layerhooks::chat_invite::capture_query_id_from_webview",
            target: "query_id",
        },
        _ => return None,
    })
}

/// The requirement function for a hook that only requires a field to be present
/// (canonical optional -> profile mandatory). It takes `(present, value)` and
/// returns the value, failing closed when absent.
///
/// Hooks with any other shape return `None`, so their layer stays fail-closed.
#[must_use]
pub fn field_require(name: &str) -> Option<&'static str> {
    Some(match name {
        "layerRequireLegacyEphemeralMessagePeer" => {
            "rtd_layerhooks::ephemeral::require_message_peer"
        }
        "layerRequireLegacyEphemeralSendPeer" => "rtd_layerhooks::ephemeral::require_send_peer",
        "layerRequireLegacyEphemeralDeletePeer" => "rtd_layerhooks::ephemeral::require_delete_peer",
        _ => return None,
    })
}

/// The names of the implemented hooks, for the hooks crate to assert against.
#[must_use]
pub fn implemented_names() -> Vec<&'static str> {
    HOOKS
        .iter()
        .filter(|spec| spec.status == Status::Implemented)
        .map(|spec| spec.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        Direction, HOOKS, LEGACY_HOOKS, Status, implemented_names, legacy_for_wire, legacy_rows,
        lookup,
    };

    #[test]
    fn names_are_unique_and_sorted() {
        for pair in HOOKS.windows(2) {
            assert!(
                pair[0].name < pair[1].name,
                "registry must be sorted: {} then {}",
                pair[0].name,
                pair[1].name
            );
        }
    }

    #[test]
    fn lookup_finds_registered_and_rejects_unknown() {
        assert!(lookup("layerBotVerificationText").is_some());
        assert!(lookup("layerNoSuchHook").is_none());
    }

    #[test]
    fn direction_covers_obligation_directions() {
        assert!(Direction::Both.covers("canonical-to-profile"));
        assert!(Direction::Both.covers("profile-to-canonical"));
        assert!(Direction::CanonicalToProfile.covers("canonical-to-profile"));
        assert!(!Direction::CanonicalToProfile.covers("profile-to-canonical"));
        assert!(!Direction::ProfileToCanonical.covers("canonical-to-profile"));
    }

    #[test]
    fn legacy_hooks_are_sorted_unique_and_looked_up() {
        assert_eq!(LEGACY_HOOKS.len(), 17);
        for pair in LEGACY_HOOKS.windows(2) {
            assert!(
                pair[0].wire_id < pair[1].wire_id,
                "legacy hooks must be sorted by wire id"
            );
        }
        for hook in LEGACY_HOOKS {
            assert_eq!(
                legacy_for_wire(hook.wire_id).map(|h| h.name),
                Some(hook.name)
            );
            assert!(hook.decode.starts_with("rtd_layerhooks::"));
            assert!(hook.encode.starts_with("rtd_layerhooks::"));
        }
        // inputPasskeyCredentialFirebasePNV has no legacy hook.
        assert!(legacy_for_wire(0x5b1c_cb28).is_none());
    }

    #[test]
    fn legacy_rows_is_only_the_inline_keyboard_hook() {
        let shape = legacy_rows("layerAdaptLegacyInlineKeyboardRows").unwrap();
        assert_eq!(
            shape.canonical,
            super::LegacyCanonical::KeyboardInlineButton
        );
        assert!(legacy_rows("layerAdaptLegacyKeyboardInlineButtonURL").is_none());
        // Every inline-canonical hook has a distinct variant for the generated
        // encode match.
        let inline: Vec<&str> = LEGACY_HOOKS
            .iter()
            .filter(|hook| hook.canonical == super::LegacyCanonical::KeyboardInlineButton)
            .map(|hook| hook.variant)
            .collect();
        assert_eq!(inline.len(), 11);
        for (index, variant) in inline.iter().enumerate() {
            assert!(variant.starts_with("InlineButtonType") || variant.starts_with("InputInline"));
            assert!(
                !inline[..index].contains(variant),
                "duplicate variant {variant}"
            );
        }
    }

    #[test]
    fn implemented_names_match_status() {
        let expected: Vec<&str> = HOOKS
            .iter()
            .filter(|spec| spec.status == Status::Implemented)
            .map(|spec| spec.name)
            .collect();
        assert_eq!(implemented_names(), expected);
    }
}
