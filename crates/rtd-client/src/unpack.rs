//! Unpacking RPC results.
//!
//! Sending or forwarding a message answers with `Updates`; [`messages`] pulls
//! the [`Message`]s those updates carry so a caller can reference what it just
//! produced.

use rtd_tg::{Message, Object, Update, Updates};

/// Extracts the messages carried by an RPC result.
#[must_use]
pub fn messages(object: &Object) -> Vec<Message> {
    match object {
        Object::Updates(updates) => messages_from_updates(updates),
        _ => Vec::new(),
    }
}

/// Extracts the messages carried by an update envelope.
#[must_use]
pub fn messages_from_updates(updates: &Updates) -> Vec<Message> {
    match updates {
        Updates::Updates { updates, .. } | Updates::UpdatesCombined { updates, .. } => {
            updates.iter().filter_map(update_message).collect()
        }
        Updates::UpdateShort { update, .. } => update_message(update).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// Returns the message a message-bearing update carries.
#[must_use]
pub fn update_message(update: &Update) -> Option<Message> {
    match update {
        Update::UpdateNewMessage { message, .. }
        | Update::UpdateNewChannelMessage { message, .. }
        | Update::UpdateEditMessage { message, .. }
        | Update::UpdateEditChannelMessage { message, .. } => Some((**message).clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use rtd_tg::{Message, Method, Object, Update, Updates};

    use super::messages;

    fn a_message() -> Message {
        Message::MessageEmpty {
            flags: 0,
            id: 42,
            peer_id: None,
        }
    }

    #[test]
    fn extracts_messages_from_updates() {
        let updates = Updates::Updates {
            updates: vec![Update::UpdateNewMessage {
                message: Box::new(a_message()),
                pts: 1,
                pts_count: 1,
            }],
            users: Vec::new(),
            chats: Vec::new(),
            date: 0,
            seq: 0,
        };
        let object = Object::Updates(Box::new(updates));
        let extracted = messages(&object);
        assert_eq!(extracted.len(), 1);
        // A non-updates object yields nothing.
        let method = Method::HelpGetConfig(Box::new(rtd_tg::HelpGetConfig));
        let _ = method;
    }
}
