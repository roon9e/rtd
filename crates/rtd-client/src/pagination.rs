//! Paginated query iterators.
//!
//! [`HistoryIter`] pages through `messages.getHistory`; [`fetch_history`] drains
//! it into a single vector. Each `next` returns one page, so a caller can stop
//! early.

use rtd_tg::{
    ChannelParticipant, ChannelParticipantsFilter, ChannelsChannelParticipants,
    ChannelsGetParticipants, Chat, ContactsBlocked, ContactsGetBlocked, Dialog, InputChannel,
    InputPeer, InputUser, Message, MessagesDialogs, MessagesGetDialogs, MessagesGetHistory,
    MessagesMessages, Method, Object, Peer, PeerBlocked, Photo, PhotosGetUserPhotos, PhotosPhotos,
    User,
};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// Default page size.
const DEFAULT_BATCH: i32 = 100;

/// An iterator over a peer's message history.
#[derive(Debug, Clone)]
pub struct HistoryIter {
    peer: InputPeer,
    offset_id: i32,
    remaining: i32,
    batch: i32,
    done: bool,
}

impl HistoryIter {
    /// Creates an iterator over all of `peer`'s history.
    #[must_use]
    pub fn new(peer: InputPeer) -> Self {
        Self {
            peer,
            offset_id: 0,
            remaining: i32::MAX,
            batch: DEFAULT_BATCH,
            done: false,
        }
    }

    /// Bounds the total number of messages returned.
    #[must_use]
    pub fn with_limit(mut self, limit: i32) -> Self {
        self.remaining = limit;
        self
    }

    /// Overrides the page size.
    #[must_use]
    pub fn with_batch(mut self, batch: i32) -> Self {
        self.batch = batch.max(1);
        self
    }

    /// Fetches the next page, or `None` when the history is exhausted.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with an unexpected constructor.
    pub fn next<T: Transport>(&mut self, client: &mut Client<T>) -> Result<Option<Vec<Message>>> {
        if self.done || self.remaining <= 0 {
            return Ok(None);
        }
        let limit = self.batch.min(self.remaining);
        let query = Method::MessagesGetHistory(Box::new(MessagesGetHistory {
            peer: Box::new(self.peer.clone()),
            offset_id: self.offset_id,
            offset_date: 0,
            add_offset: 0,
            limit,
            max_id: 0,
            min_id: 0,
            hash: 0,
        }));
        let messages = match client.invoke(query)? {
            Object::MessagesMessages(page) => extract_messages(*page),
            other => {
                return Err(Error::Unexpected(format!(
                    "messages.getHistory returned unexpected constructor {:#010x}",
                    other.type_id()
                )));
            }
        };
        if messages.is_empty() {
            self.done = true;
            return Ok(None);
        }
        if let Some(last) = messages.last() {
            self.offset_id = message_id(last);
        }
        self.remaining -= messages.len() as i32;
        if messages.len() < limit as usize {
            self.done = true;
        }
        Ok(Some(messages))
    }
}

/// Drains a history iterator into one vector.
///
/// # Errors
///
/// Returns the iterator's error.
pub fn fetch_history<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    limit: i32,
) -> Result<Vec<Message>> {
    let mut iter = HistoryIter::new(peer).with_limit(limit);
    let mut all = Vec::new();
    while let Some(page) = iter.next(client)? {
        all.extend(page);
    }
    Ok(all)
}

/// An iterator over the account's dialogs.
#[derive(Debug, Clone)]
pub struct DialogsIter {
    offset_id: i32,
    offset_date: i32,
    offset_peer: InputPeer,
    remaining: i32,
    batch: i32,
    done: bool,
}

impl DialogsIter {
    /// Creates an iterator over all dialogs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset_id: 0,
            offset_date: 0,
            offset_peer: InputPeer::InputPeerEmpty,
            remaining: i32::MAX,
            batch: DEFAULT_BATCH,
            done: false,
        }
    }

    /// Bounds the total number of dialogs returned.
    #[must_use]
    pub fn with_limit(mut self, limit: i32) -> Self {
        self.remaining = limit;
        self
    }

    /// Overrides the page size.
    #[must_use]
    pub fn with_batch(mut self, batch: i32) -> Self {
        self.batch = batch.max(1);
        self
    }

    /// Fetches the next page of dialogs, or `None` when exhausted.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with an unexpected constructor.
    pub fn next<T: Transport>(&mut self, client: &mut Client<T>) -> Result<Option<Vec<Dialog>>> {
        if self.done || self.remaining <= 0 {
            return Ok(None);
        }
        let limit = self.batch.min(self.remaining);
        let query = Method::MessagesGetDialogs(Box::new(MessagesGetDialogs {
            flags: 0,
            exclude_pinned: false,
            folder_id: None,
            offset_date: self.offset_date,
            offset_id: self.offset_id,
            offset_peer: Box::new(self.offset_peer.clone()),
            limit,
            hash: 0,
        }));
        let (dialogs, users, chats) = match client.invoke(query)? {
            Object::MessagesDialogs(page) => extract_dialogs(*page),
            other => {
                return Err(Error::Unexpected(format!(
                    "messages.getDialogs returned unexpected constructor {:#010x}",
                    other.type_id()
                )));
            }
        };
        if dialogs.is_empty() {
            self.done = true;
            return Ok(None);
        }
        match dialogs.last() {
            Some(Dialog::Dialog {
                peer, top_message, ..
            }) => {
                self.offset_id = *top_message;
                self.offset_date = 0;
                self.offset_peer =
                    input_peer(peer, &users, &chats).unwrap_or(InputPeer::InputPeerEmpty);
            }
            _ => self.done = true,
        }
        self.remaining -= dialogs.len() as i32;
        if dialogs.len() < limit as usize {
            self.done = true;
        }
        Ok(Some(dialogs))
    }
}

impl Default for DialogsIter {
    fn default() -> Self {
        Self::new()
    }
}

/// Drains a dialogs iterator into one vector.
///
/// # Errors
///
/// Returns the iterator's error.
pub fn fetch_dialogs<T: Transport>(client: &mut Client<T>, limit: i32) -> Result<Vec<Dialog>> {
    let mut iter = DialogsIter::new().with_limit(limit);
    let mut all = Vec::new();
    while let Some(page) = iter.next(client)? {
        all.extend(page);
    }
    Ok(all)
}

/// An iterator over a channel's participants.
#[derive(Debug, Clone)]
pub struct ParticipantsIter {
    channel: InputChannel,
    offset: i32,
    remaining: i32,
    batch: i32,
    done: bool,
}

impl ParticipantsIter {
    /// Creates an iterator over all of `channel`'s participants.
    #[must_use]
    pub fn new(channel: InputChannel) -> Self {
        Self {
            channel,
            offset: 0,
            remaining: i32::MAX,
            batch: DEFAULT_BATCH,
            done: false,
        }
    }

    /// Bounds the total number of participants returned.
    #[must_use]
    pub fn with_limit(mut self, limit: i32) -> Self {
        self.remaining = limit;
        self
    }

    /// Fetches the next page of participants, or `None` when exhausted.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with an unexpected constructor.
    pub fn next<T: Transport>(
        &mut self,
        client: &mut Client<T>,
    ) -> Result<Option<Vec<ChannelParticipant>>> {
        if self.done || self.remaining <= 0 {
            return Ok(None);
        }
        let limit = self.batch.min(self.remaining);
        let query = Method::ChannelsGetParticipants(Box::new(ChannelsGetParticipants {
            channel: Box::new(self.channel.clone()),
            filter: Box::new(ChannelParticipantsFilter::ChannelParticipantsRecent),
            offset: self.offset,
            limit,
            hash: 0,
        }));
        let participants = match client.invoke(query)? {
            Object::ChannelsChannelParticipants(page) => match *page {
                ChannelsChannelParticipants::ChannelParticipants { participants, .. } => {
                    participants
                }
                ChannelsChannelParticipants::ChannelParticipantsNotModified => Vec::new(),
            },
            other => {
                return Err(Error::Unexpected(format!(
                    "channels.getParticipants returned unexpected constructor {:#010x}",
                    other.type_id()
                )));
            }
        };
        if participants.is_empty() {
            self.done = true;
            return Ok(None);
        }
        self.offset += participants.len() as i32;
        self.remaining -= participants.len() as i32;
        if participants.len() < limit as usize {
            self.done = true;
        }
        Ok(Some(participants))
    }
}

/// Drains a participants iterator into one vector.
///
/// # Errors
///
/// Returns the iterator's error.
pub fn fetch_participants<T: Transport>(
    client: &mut Client<T>,
    channel: InputChannel,
    limit: i32,
) -> Result<Vec<ChannelParticipant>> {
    let mut iter = ParticipantsIter::new(channel).with_limit(limit);
    let mut all = Vec::new();
    while let Some(page) = iter.next(client)? {
        all.extend(page);
    }
    Ok(all)
}

/// An iterator over a user's profile photos.
#[derive(Debug, Clone)]
pub struct PhotosIter {
    user: InputUser,
    offset: i32,
    remaining: i32,
    batch: i32,
    done: bool,
}

impl PhotosIter {
    /// Creates an iterator over all of `user`'s photos.
    #[must_use]
    pub fn new(user: InputUser) -> Self {
        Self {
            user,
            offset: 0,
            remaining: i32::MAX,
            batch: DEFAULT_BATCH,
            done: false,
        }
    }

    /// Bounds the total number of photos returned.
    #[must_use]
    pub fn with_limit(mut self, limit: i32) -> Self {
        self.remaining = limit;
        self
    }

    /// Fetches the next page of photos, or `None` when exhausted.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with an unexpected constructor.
    pub fn next<T: Transport>(&mut self, client: &mut Client<T>) -> Result<Option<Vec<Photo>>> {
        if self.done || self.remaining <= 0 {
            return Ok(None);
        }
        let limit = self.batch.min(self.remaining);
        let query = Method::PhotosGetUserPhotos(Box::new(PhotosGetUserPhotos {
            user_id: Box::new(self.user.clone()),
            offset: self.offset,
            max_id: 0,
            limit,
        }));
        let photos = match client.invoke(query)? {
            Object::PhotosPhotos(page) => match *page {
                PhotosPhotos::Photos { photos, .. } | PhotosPhotos::PhotosSlice { photos, .. } => {
                    photos
                }
            },
            other => {
                return Err(Error::Unexpected(format!(
                    "photos.getUserPhotos returned unexpected constructor {:#010x}",
                    other.type_id()
                )));
            }
        };
        if photos.is_empty() {
            self.done = true;
            return Ok(None);
        }
        self.offset += photos.len() as i32;
        self.remaining -= photos.len() as i32;
        if photos.len() < limit as usize {
            self.done = true;
        }
        Ok(Some(photos))
    }
}

/// An iterator over blocked peers.
#[derive(Debug, Clone)]
pub struct BlockedIter {
    offset: i32,
    remaining: i32,
    batch: i32,
    done: bool,
}

impl BlockedIter {
    /// Creates an iterator over all blocked peers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset: 0,
            remaining: i32::MAX,
            batch: DEFAULT_BATCH,
            done: false,
        }
    }

    /// Bounds the total number of peers returned.
    #[must_use]
    pub fn with_limit(mut self, limit: i32) -> Self {
        self.remaining = limit;
        self
    }

    /// Fetches the next page of blocked peers, or `None` when exhausted.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with an unexpected constructor.
    pub fn next<T: Transport>(
        &mut self,
        client: &mut Client<T>,
    ) -> Result<Option<Vec<PeerBlocked>>> {
        if self.done || self.remaining <= 0 {
            return Ok(None);
        }
        let limit = self.batch.min(self.remaining);
        let query = Method::ContactsGetBlocked(Box::new(ContactsGetBlocked {
            flags: 0,
            my_stories_from: false,
            offset: self.offset,
            limit,
        }));
        let blocked = match client.invoke(query)? {
            Object::ContactsBlocked(page) => match *page {
                ContactsBlocked::Blocked { blocked, .. }
                | ContactsBlocked::BlockedSlice { blocked, .. } => blocked,
            },
            other => {
                return Err(Error::Unexpected(format!(
                    "contacts.getBlocked returned unexpected constructor {:#010x}",
                    other.type_id()
                )));
            }
        };
        if blocked.is_empty() {
            self.done = true;
            return Ok(None);
        }
        self.offset += blocked.len() as i32;
        self.remaining -= blocked.len() as i32;
        if blocked.len() < limit as usize {
            self.done = true;
        }
        Ok(Some(blocked))
    }
}

impl Default for BlockedIter {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracts dialogs, users and chats from a page.
fn extract_dialogs(page: MessagesDialogs) -> (Vec<Dialog>, Vec<User>, Vec<Chat>) {
    match page {
        MessagesDialogs::Dialogs {
            dialogs,
            users,
            chats,
            ..
        }
        | MessagesDialogs::DialogsSlice {
            dialogs,
            users,
            chats,
            ..
        } => (dialogs, users, chats),
        MessagesDialogs::DialogsNotModified { .. } => (Vec::new(), Vec::new(), Vec::new()),
    }
}

/// Converts a `Peer` to an `InputPeer` using the page's users and chats.
fn input_peer(peer: &Peer, users: &[User], chats: &[Chat]) -> Option<InputPeer> {
    match peer {
        Peer::PeerUser { user_id } => users.iter().find_map(|user| match user {
            User::User {
                id, access_hash, ..
            } if id == user_id => Some(InputPeer::InputPeerUser {
                user_id: *user_id,
                access_hash: access_hash.unwrap_or(0),
            }),
            _ => None,
        }),
        Peer::PeerChat { chat_id } => Some(InputPeer::InputPeerChat { chat_id: *chat_id }),
        Peer::PeerChannel { channel_id } => chats.iter().find_map(|chat| match chat {
            Chat::Channel {
                id, access_hash, ..
            } if id == channel_id => Some(InputPeer::InputPeerChannel {
                channel_id: *channel_id,
                access_hash: access_hash.unwrap_or(0),
            }),
            _ => None,
        }),
    }
}

/// Extracts the messages vector from a page.
fn extract_messages(page: MessagesMessages) -> Vec<Message> {
    match page {
        MessagesMessages::Messages { messages, .. }
        | MessagesMessages::MessagesSlice { messages, .. }
        | MessagesMessages::ChannelMessages { messages, .. } => messages,
        MessagesMessages::MessagesNotModified { .. } => Vec::new(),
    }
}

/// Returns a message's id (0 for a constructor without one).
fn message_id(message: &Message) -> i32 {
    match message {
        Message::Message { id, .. }
        | Message::MessageService { id, .. }
        | Message::MessageEmpty { id, .. } => *id,
    }
}

#[cfg(test)]
mod tests {
    use super::HistoryIter;
    use rtd_tg::InputPeer;

    #[test]
    fn iterators_start_at_offset_zero() {
        let iter = HistoryIter::new(InputPeer::InputPeerEmpty).with_limit(10);
        assert_eq!(iter.offset_id, 0);
        assert_eq!(iter.remaining, 10);
        let dialogs = super::DialogsIter::new().with_limit(5);
        assert_eq!(dialogs.offset_id, 0);
        assert_eq!(dialogs.remaining, 5);
    }
}
