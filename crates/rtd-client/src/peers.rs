//! Peer resolution and caching.
//!
//! [`PeerManager`] resolves usernames to a peer's input form with
//! `contacts.resolveUsername` and caches the result.

use std::collections::HashMap;
use std::sync::Mutex;

use rtd_tg::{Chat, ContactsResolveUsername, InputPeer, Method, Object, Peer};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// A resolved peer with the identity needed to reference it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedPeer {
    /// A user.
    User {
        /// The user id.
        user_id: i64,
        /// The user's access hash.
        access_hash: i64,
    },
    /// A legacy chat.
    Chat {
        /// The chat id.
        chat_id: i64,
    },
    /// A channel or supergroup.
    Channel {
        /// The channel id.
        channel_id: i64,
        /// The channel's access hash.
        access_hash: i64,
    },
}

impl ResolvedPeer {
    /// Returns the peer's [`InputPeer`] form.
    #[must_use]
    pub fn input_peer(&self) -> InputPeer {
        match *self {
            Self::User {
                user_id,
                access_hash,
            } => InputPeer::InputPeerUser {
                user_id,
                access_hash,
            },
            Self::Chat { chat_id } => InputPeer::InputPeerChat { chat_id },
            Self::Channel {
                channel_id,
                access_hash,
            } => InputPeer::InputPeerChannel {
                channel_id,
                access_hash,
            },
        }
    }
}

/// A cache of resolved peers keyed by username.
#[derive(Debug, Default)]
pub struct PeerManager {
    cache: Mutex<HashMap<String, ResolvedPeer>>,
}

impl PeerManager {
    /// Creates an empty manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a previously resolved peer, if cached.
    #[must_use]
    pub fn get(&self, username: &str) -> Option<ResolvedPeer> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(username)
            .copied()
    }

    /// Resolves `username`, caching and returning the result.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers without an access hash or an unexpected
    /// constructor.
    pub fn resolve<T: Transport>(
        &self,
        client: &mut Client<T>,
        username: &str,
    ) -> Result<ResolvedPeer> {
        let query = Method::ContactsResolveUsername(Box::new(ContactsResolveUsername {
            flags: 0,
            username: username.to_owned(),
            referer: None,
        }));
        let resolved = match client.invoke(query)? {
            Object::ContactsResolvedPeer(resolved) => *resolved,
            other => {
                return Err(Error::Unexpected(format!(
                    "contacts.resolveUsername returned unexpected constructor {:#010x}",
                    other.type_id()
                )));
            }
        };
        let peer = match &*resolved.peer {
            Peer::PeerUser { user_id } => {
                let access_hash = resolved
                    .users
                    .iter()
                    .find_map(|user| match user {
                        rtd_tg::User::User {
                            id, access_hash, ..
                        } if id == user_id => *access_hash,
                        _ => None,
                    })
                    .ok_or_else(|| {
                        Error::Unexpected("resolved user has no access hash".to_owned())
                    })?;
                ResolvedPeer::User {
                    user_id: *user_id,
                    access_hash,
                }
            }
            Peer::PeerChat { chat_id } => ResolvedPeer::Chat { chat_id: *chat_id },
            Peer::PeerChannel { channel_id } => {
                let access_hash = resolved
                    .chats
                    .iter()
                    .find_map(|chat| match chat {
                        Chat::Channel {
                            id, access_hash, ..
                        } if id == channel_id => *access_hash,
                        _ => None,
                    })
                    .ok_or_else(|| {
                        Error::Unexpected("resolved channel has no access hash".to_owned())
                    })?;
                ResolvedPeer::Channel {
                    channel_id: *channel_id,
                    access_hash,
                }
            }
        };
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(username.to_owned(), peer);
        Ok(peer)
    }
}

#[cfg(test)]
mod tests {
    use super::ResolvedPeer;
    use rtd_tg::InputPeer;

    #[test]
    fn resolved_peer_maps_to_input_peer() {
        let user = ResolvedPeer::User {
            user_id: 7,
            access_hash: 9,
        };
        assert_eq!(
            user.input_peer(),
            InputPeer::InputPeerUser {
                user_id: 7,
                access_hash: 9,
            }
        );
        assert_eq!(
            ResolvedPeer::Chat { chat_id: 3 }.input_peer(),
            InputPeer::InputPeerChat { chat_id: 3 }
        );
    }
}
