//! Peer resolution and caching.
//!
//! [`PeerManager`] resolves usernames to a peer's input form with
//! `contacts.resolveUsername` and caches the result.

use std::collections::HashMap;
use std::sync::Mutex;

use rtd_tg::{
    Chat as TgChat, ContactsResolveUsername, InputPeer, Method, Object, Peer, Updates,
    User as TgUser,
};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// A storage key identifying a peer entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerKey {
    /// The entity prefix (`users`, `chats`, `channel`).
    pub prefix: String,
    /// The entity id.
    pub id: i64,
}

impl PeerKey {
    /// Creates a key.
    #[must_use]
    pub fn new(prefix: impl Into<String>, id: i64) -> Self {
        Self {
            prefix: prefix.into(),
            id,
        }
    }
}

/// A stored peer value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerValue {
    /// The peer's access hash.
    pub access_hash: i64,
}

/// A synchronous store for resolved peer access hashes, phone mappings and the
/// contacts hash.
pub trait PeerStorage: Send + Sync {
    /// Saves `value` under `key`.
    fn save(&self, key: PeerKey, value: PeerValue);
    /// Finds the value stored under `key`.
    fn find(&self, key: &PeerKey) -> Option<PeerValue>;
    /// Saves a phone-number mapping to `key`.
    fn save_phone(&self, phone: &str, key: PeerKey);
    /// Finds the phone-number mapping.
    fn find_phone(&self, phone: &str) -> Option<(PeerKey, PeerValue)>;
    /// Returns the stored contacts hash.
    fn contacts_hash(&self) -> i64;
    /// Stores the contacts hash.
    fn save_contacts_hash(&self, hash: i64);
}

/// An in-memory [`PeerStorage`].
#[derive(Debug, Default)]
pub struct MemoryPeerStorage {
    entries: Mutex<HashMap<PeerKey, PeerValue>>,
    phones: Mutex<HashMap<String, PeerKey>>,
    contacts_hash: std::sync::atomic::AtomicI64,
}

impl MemoryPeerStorage {
    /// Creates an empty storage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl PeerStorage for MemoryPeerStorage {
    fn save(&self, key: PeerKey, value: PeerValue) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, value);
    }

    fn find(&self, key: &PeerKey) -> Option<PeerValue> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
            .copied()
    }

    fn save_phone(&self, phone: &str, key: PeerKey) {
        self.phones
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(phone.to_owned(), key);
    }

    fn find_phone(&self, phone: &str) -> Option<(PeerKey, PeerValue)> {
        let key = self
            .phones
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(phone)
            .cloned()?;
        let value = self.find(&key)?;
        Some((key, value))
    }

    fn contacts_hash(&self) -> i64 {
        self.contacts_hash.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn save_contacts_hash(&self, hash: i64) {
        self.contacts_hash
            .store(hash, std::sync::atomic::Ordering::SeqCst);
    }
}

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

/// A resolved user with its access hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct User {
    /// The user id.
    pub id: i64,
    /// The user's access hash.
    pub access_hash: i64,
}

impl User {
    /// Creates a user.
    #[must_use]
    pub fn new(id: i64, access_hash: i64) -> Self {
        Self { id, access_hash }
    }

    /// Returns the user's [`InputPeer`].
    #[must_use]
    pub fn input_peer(&self) -> InputPeer {
        InputPeer::InputPeerUser {
            user_id: self.id,
            access_hash: self.access_hash,
        }
    }
}

/// A legacy chat (which carries no access hash).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chat {
    /// The chat id.
    pub id: i64,
}

impl Chat {
    /// Creates a chat.
    #[must_use]
    pub fn new(id: i64) -> Self {
        Self { id }
    }

    /// Returns the chat's [`InputPeer`].
    #[must_use]
    pub fn input_peer(&self) -> InputPeer {
        InputPeer::InputPeerChat { chat_id: self.id }
    }
}

/// A channel with its access hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channel {
    /// The channel id.
    pub id: i64,
    /// The channel's access hash.
    pub access_hash: i64,
}

impl Channel {
    /// Creates a channel.
    #[must_use]
    pub fn new(id: i64, access_hash: i64) -> Self {
        Self { id, access_hash }
    }

    /// Returns the channel's [`InputPeer`].
    #[must_use]
    pub fn input_peer(&self) -> InputPeer {
        InputPeer::InputPeerChannel {
            channel_id: self.id,
            access_hash: self.access_hash,
        }
    }
}

/// A supergroup (a megagroup channel) with its access hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Supergroup {
    /// The supergroup id.
    pub id: i64,
    /// The supergroup's access hash.
    pub access_hash: i64,
}

impl Supergroup {
    /// Creates a supergroup.
    #[must_use]
    pub fn new(id: i64, access_hash: i64) -> Self {
        Self { id, access_hash }
    }

    /// Returns the supergroup's [`InputPeer`].
    #[must_use]
    pub fn input_peer(&self) -> InputPeer {
        InputPeer::InputPeerChannel {
            channel_id: self.id,
            access_hash: self.access_hash,
        }
    }
}

/// The kind of a cached peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeerKind {
    /// A user.
    User,
    /// A legacy chat.
    Chat,
    /// A channel or supergroup.
    Channel,
}

impl PeerKind {
    fn tag(self) -> u8 {
        match self {
            Self::User => 0,
            Self::Chat => 1,
            Self::Channel => 2,
        }
    }
}

/// A cache of resolved peers keyed by username and by `(kind, id)`.
#[derive(Debug, Default)]
pub struct PeerManager {
    cache: Mutex<HashMap<String, ResolvedPeer>>,
    entities: Mutex<HashMap<(u8, i64), ResolvedPeer>>,
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

    /// Returns a previously cached entity by kind and id.
    #[must_use]
    pub fn get_by_id(&self, kind: PeerKind, id: i64) -> Option<ResolvedPeer> {
        self.entities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(kind.tag(), id))
            .copied()
    }

    /// Caches users and chats carried by an update envelope.
    ///
    /// Bare short updates carry no entities, so they are ignored.
    pub fn apply_updates(&self, updates: &Updates) {
        let (users, chats) = match updates {
            Updates::Updates { users, chats, .. }
            | Updates::UpdatesCombined { users, chats, .. } => (users, chats),
            _ => return,
        };
        self.cache_entities(users, chats);
    }

    /// Caches users and chats by kind and id (and username, when present).
    pub fn cache_entities(&self, users: &[TgUser], chats: &[TgChat]) {
        let mut entities = self
            .entities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut by_name = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for user in users {
            if let TgUser::User {
                id,
                access_hash,
                username,
                ..
            } = user
                && let Some(access_hash) = access_hash
            {
                let peer = ResolvedPeer::User {
                    user_id: *id,
                    access_hash: *access_hash,
                };
                entities.insert((PeerKind::User.tag(), *id), peer);
                if let Some(name) = username {
                    by_name.insert(name.to_lowercase(), peer);
                }
            }
        }
        for chat in chats {
            match chat {
                TgChat::Channel {
                    id,
                    access_hash: Some(access_hash),
                    username,
                    ..
                } => {
                    let peer = ResolvedPeer::Channel {
                        channel_id: *id,
                        access_hash: *access_hash,
                    };
                    entities.insert((PeerKind::Channel.tag(), *id), peer);
                    if let Some(name) = username {
                        by_name.insert(name.to_lowercase(), peer);
                    }
                }
                TgChat::Chat { id, .. } => {
                    entities.insert(
                        (PeerKind::Chat.tag(), *id),
                        ResolvedPeer::Chat { chat_id: *id },
                    );
                }
                _ => {}
            }
        }
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
                        TgChat::Channel {
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

    #[test]
    fn entity_wrappers_produce_input_peers() {
        use super::{Channel, Chat, Supergroup, User};

        assert_eq!(
            User::new(1, 2).input_peer(),
            InputPeer::InputPeerUser {
                user_id: 1,
                access_hash: 2,
            }
        );
        assert_eq!(
            Chat::new(3).input_peer(),
            InputPeer::InputPeerChat { chat_id: 3 }
        );
        assert_eq!(
            Channel::new(4, 5).input_peer(),
            InputPeer::InputPeerChannel {
                channel_id: 4,
                access_hash: 5,
            }
        );
        assert_eq!(
            Supergroup::new(6, 7).input_peer(),
            InputPeer::InputPeerChannel {
                channel_id: 6,
                access_hash: 7,
            }
        );
    }

    #[test]
    fn memory_storage_round_trips() {
        use super::{MemoryPeerStorage, PeerKey, PeerStorage, PeerValue};

        let storage = MemoryPeerStorage::new();
        let key = PeerKey::new("users", 42);
        assert!(storage.find(&key).is_none());
        storage.save(key.clone(), PeerValue { access_hash: 7 });
        assert_eq!(storage.find(&key), Some(PeerValue { access_hash: 7 }));

        storage.save_phone("+100", key.clone());
        assert_eq!(
            storage.find_phone("+100"),
            Some((key.clone(), PeerValue { access_hash: 7 }))
        );
        assert!(storage.find_phone("+200").is_none());

        assert_eq!(storage.contacts_hash(), 0);
        storage.save_contacts_hash(123);
        assert_eq!(storage.contacts_hash(), 123);
    }

    /// Builds a minimal `User` with the given identity fields set.
    fn tg_user(id: i64, access_hash: Option<i64>, username: Option<&str>) -> rtd_tg::User {
        rtd_tg::User::User {
            flags: 0,
            self_: false,
            contact: false,
            mutual_contact: false,
            deleted: false,
            bot: false,
            bot_chat_history: false,
            bot_nochats: false,
            verified: false,
            restricted: false,
            min: false,
            bot_inline_geo: false,
            support: false,
            scam: false,
            apply_min_photo: false,
            fake: false,
            bot_attach_menu: false,
            premium: false,
            attach_menu_enabled: false,
            flags2: 0,
            bot_can_edit: false,
            close_friend: false,
            stories_hidden: false,
            stories_unavailable: false,
            contact_require_premium: false,
            bot_business: false,
            bot_has_main_app: false,
            bot_forum_view: false,
            bot_forum_can_manage_topics: false,
            bot_can_manage_bots: false,
            bot_guestchat: false,
            bot_guard: false,
            id,
            access_hash,
            first_name: None,
            last_name: None,
            username: username.map(str::to_owned),
            phone: None,
            photo: None,
            status: None,
            bot_info_version: None,
            restriction_reason: None,
            bot_inline_placeholder: None,
            lang_code: None,
            emoji_status: None,
            usernames: None,
            stories_max_id: None,
            color: None,
            profile_color: None,
            bot_active_users: None,
            bot_verification_icon: None,
            send_paid_messages_stars: None,
            linked_community_id: None,
        }
    }

    #[test]
    fn caches_and_refreshes_entities() {
        use super::{PeerKind, PeerManager, ResolvedPeer};

        let manager = PeerManager::new();
        manager.cache_entities(&[tg_user(7, Some(9), Some("Alice"))], &[]);

        let expected = ResolvedPeer::User {
            user_id: 7,
            access_hash: 9,
        };
        assert_eq!(manager.get("alice"), Some(expected));
        assert_eq!(manager.get_by_id(PeerKind::User, 7), Some(expected));

        // A later update with a new access hash refreshes the cache.
        manager.cache_entities(&[tg_user(7, Some(10), Some("Alice"))], &[]);
        let refreshed = ResolvedPeer::User {
            user_id: 7,
            access_hash: 10,
        };
        assert_eq!(manager.get_by_id(PeerKind::User, 7), Some(refreshed));
        assert_eq!(manager.get("alice"), Some(refreshed));
    }
}
