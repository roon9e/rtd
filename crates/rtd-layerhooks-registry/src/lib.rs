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
        status: Status::Pending,
        signature: "layerAdaptBotGuestChatResult(_ LayerProfile, messageID InputBotInlineMessageID) (Bool, error)",
    },
    HookSpec {
        name: "layerAdaptChatInviteJoinResultToUpdates",
        direction: Direction::CanonicalToProfile,
        status: Status::Pending,
        signature: "layerAdaptChatInviteJoinResultToUpdates(_ LayerProfile, result MessagesChatInviteJoinResult) (Updates, error)",
    },
    HookSpec {
        name: "layerAdaptLegacyInlineKeyboardRows",
        direction: Direction::Both,
        status: Status::Pending,
        signature: "layerAdaptLegacyInlineKeyboardRows(_ LayerProfile, rows Vector<KeyboardButtonRow>/Vector<KeyboardInlineButtonRow>) (Vector<...>, error)",
    },
    HookSpec {
        name: "layerAdaptUpdatesToChatInviteJoinResult",
        direction: Direction::ProfileToCanonical,
        status: Status::Pending,
        signature: "layerAdaptUpdatesToChatInviteJoinResult(_ LayerProfile, updates Updates) (MessagesChatInviteJoinResult, error)",
    },
    HookSpec {
        name: "layerAdaptWebBrowserSettingsExceptionResult",
        direction: Direction::CanonicalToProfile,
        status: Status::Pending,
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
        status: Status::Pending,
        signature: "layerCaptureChatInviteJoinQueryIDFromWebView(_ LayerProfile, value MessagesChatInviteJoinResultWebView, webview WebViewResult) error",
    },
    HookSpec {
        name: "layerDefaultLegacyKeyboardButtonType",
        direction: Direction::ProfileToCanonical,
        status: Status::Pending,
        signature: "layerDefaultLegacyKeyboardButtonTypeDecode(_ LayerProfile, value *KeyboardButton) (ButtonType, error)",
    },
    HookSpec {
        name: "layerRequireCapturedChatInviteJoinQueryID",
        direction: Direction::ProfileToCanonical,
        status: Status::Pending,
        signature: "layerRequireCapturedChatInviteJoinQueryIDDecode(_ LayerProfile, value MessagesChatInviteJoinResultWebView) (int64, error)",
    },
    HookSpec {
        name: "layerRequireLegacyEphemeralDeletePeer",
        direction: Direction::CanonicalToProfile,
        status: Status::Pending,
        signature: "layerRequireLegacyEphemeralDeletePeerEncode(_ LayerProfile, _ *EphemeralDeleteMessage, present bool, peer InputPeer) (InputPeer, error)",
    },
    HookSpec {
        name: "layerRequireLegacyEphemeralMessagePeer",
        direction: Direction::CanonicalToProfile,
        status: Status::Pending,
        signature: "layerRequireLegacyEphemeralMessagePeerEncode(_ LayerProfile, _ *EphemeralMessage, present bool, peer Peer) (Peer, error)",
    },
    HookSpec {
        name: "layerRequireLegacyEphemeralSendPeer",
        direction: Direction::CanonicalToProfile,
        status: Status::Pending,
        signature: "layerRequireLegacyEphemeralSendPeerEncode(_ LayerProfile, _ *EphemeralSendMessage, present bool, peer InputPeer) (InputPeer, error)",
    },
];

/// Looks up a hook signature by name.
#[must_use]
pub fn lookup(name: &str) -> Option<&'static HookSpec> {
    HOOKS.iter().find(|spec| spec.name == name)
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
    use super::{Direction, HOOKS, Status, implemented_names, lookup};

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
    fn implemented_names_match_status() {
        let expected: Vec<&str> = HOOKS
            .iter()
            .filter(|spec| spec.status == Status::Implemented)
            .map(|spec| spec.name)
            .collect();
        assert_eq!(implemented_names(), expected);
    }
}
