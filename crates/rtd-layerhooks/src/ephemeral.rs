//! Layer-228 ephemeral peer requirement hooks.
//!
//! The historical ephemeral shapes require an explicit peer, while the canonical
//! shapes made it optional. Encoding a value without the peer cannot be
//! represented and fails closed rather than dropping it. These are field-shaped
//! requirement hooks: they return the peer when present and fail closed when not.

use rtd_tg::{InputPeer, Peer};

use crate::error::{HookError, Result};

/// Canonical to profile for `ephemeralMessage`: the older shape requires
/// `peer_id`.
///
/// # Errors
///
/// Returns [`HookError::Missing`] when the peer is absent.
pub fn require_message_peer(present: bool, peer: Option<&Peer>) -> Result<&Peer> {
    match peer {
        Some(peer) if present => Ok(peer),
        _ => Err(HookError::Missing("ephemeralMessage.peer_id")),
    }
}

/// Canonical to profile for `ephemeral.sendMessage`: the older shape requires
/// `peer`.
///
/// # Errors
///
/// Returns [`HookError::Missing`] when the peer is absent.
pub fn require_send_peer(present: bool, peer: Option<&InputPeer>) -> Result<&InputPeer> {
    match peer {
        Some(peer) if present => Ok(peer),
        _ => Err(HookError::Missing("ephemeral.sendMessage.peer")),
    }
}

/// Canonical to profile for `ephemeral.deleteMessage`: the older shape requires
/// `peer`.
///
/// # Errors
///
/// Returns [`HookError::Missing`] when the peer is absent.
pub fn require_delete_peer(present: bool, peer: Option<&InputPeer>) -> Result<&InputPeer> {
    match peer {
        Some(peer) if present => Ok(peer),
        _ => Err(HookError::Missing("ephemeral.deleteMessage.peer")),
    }
}

#[cfg(test)]
mod tests {
    use rtd_tg::{InputPeer, Peer};

    use super::{require_delete_peer, require_message_peer, require_send_peer};
    use crate::error::HookError;

    fn peer() -> Peer {
        Peer::PeerUser { user_id: 7 }
    }

    #[test]
    fn present_peer_is_returned() {
        let value = peer();
        assert_eq!(require_message_peer(true, Some(&value)).unwrap(), &value);
        let input = InputPeer::InputPeerSelf;
        assert_eq!(require_send_peer(true, Some(&input)).unwrap(), &input);
        assert_eq!(require_delete_peer(true, Some(&input)).unwrap(), &input);
    }

    #[test]
    fn absent_peer_fails_closed() {
        assert!(matches!(
            require_message_peer(false, None).unwrap_err(),
            HookError::Missing(_)
        ));
        assert!(matches!(
            require_send_peer(true, None).unwrap_err(),
            HookError::Missing(_)
        ));
        assert!(matches!(
            require_delete_peer(false, Some(&InputPeer::InputPeerSelf)).unwrap_err(),
            HookError::Missing(_)
        ));
    }
}
