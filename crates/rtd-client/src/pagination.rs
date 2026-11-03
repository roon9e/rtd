//! Paginated query iterators.
//!
//! [`HistoryIter`] pages through `messages.getHistory`; [`fetch_history`] drains
//! it into a single vector. Each `next` returns one page, so a caller can stop
//! early.

use rtd_tg::{InputPeer, Message, MessagesGetHistory, MessagesMessages, Method, Object};

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
    }
}
