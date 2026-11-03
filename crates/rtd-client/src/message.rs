//! Message builders.
//!
//! [`RequestBuilder`] constructs a `messages.sendMessage` request from a peer,
//! text and options; [`send`] invokes it through a client.

use rtd_tg::{
    DocumentAttribute, InputFile, InputMedia, InputPeer, MessageEntity, MessagesSendMedia,
    MessagesSendMessage, Method, Object, ReplyMarkup,
};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// A builder for a `messages.sendMessage` request.
#[derive(Debug, Clone)]
pub struct RequestBuilder {
    peer: InputPeer,
    message: String,
    entities: Option<Vec<MessageEntity>>,
    reply_markup: Option<Box<ReplyMarkup>>,
    silent: bool,
    background: bool,
    clear_draft: bool,
    no_webpage: bool,
    noforwards: bool,
    invert_media: bool,
}

impl RequestBuilder {
    /// Creates a builder for `message` to `peer`.
    #[must_use]
    pub fn new(peer: InputPeer, message: impl Into<String>) -> Self {
        Self {
            peer,
            message: message.into(),
            entities: None,
            reply_markup: None,
            silent: false,
            background: false,
            clear_draft: false,
            no_webpage: false,
            noforwards: false,
            invert_media: false,
        }
    }

    /// Creates a builder from HTML text, parsing its entities.
    #[must_use]
    pub fn html(peer: InputPeer, html: &str) -> Self {
        let (text, entities) = crate::entity::parse_html(html);
        let builder = Self::new(peer, text);
        if entities.is_empty() {
            builder
        } else {
            builder.entities(entities)
        }
    }

    /// Creates a builder from Markdown text, parsing its entities.
    #[must_use]
    pub fn markdown(peer: InputPeer, markdown: &str) -> Self {
        let (text, entities) = crate::entity::parse_markdown(markdown);
        let builder = Self::new(peer, text);
        if entities.is_empty() {
            builder
        } else {
            builder.entities(entities)
        }
    }

    /// Sends without a notification sound.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.silent = true;
        self
    }

    /// Sends in the background.
    #[must_use]
    pub fn background(mut self) -> Self {
        self.background = true;
        self
    }

    /// Clears the draft before sending.
    #[must_use]
    pub fn clear_draft(mut self) -> Self {
        self.clear_draft = true;
        self
    }

    /// Disables the link preview.
    #[must_use]
    pub fn no_webpage(mut self) -> Self {
        self.no_webpage = true;
        self
    }

    /// Disallows forwarding.
    #[must_use]
    pub fn noforwards(mut self) -> Self {
        self.noforwards = true;
        self
    }

    /// Inverts media above the text.
    #[must_use]
    pub fn invert_media(mut self) -> Self {
        self.invert_media = true;
        self
    }

    /// Attaches message entities.
    #[must_use]
    pub fn entities(mut self, entities: Vec<MessageEntity>) -> Self {
        self.entities = Some(entities);
        self
    }

    /// Attaches a reply markup (keyboard).
    #[must_use]
    pub fn reply_markup(mut self, markup: ReplyMarkup) -> Self {
        self.reply_markup = Some(Box::new(markup));
        self
    }

    /// Attaches an inline keyboard built from rows of buttons.
    #[must_use]
    pub fn inline_keyboard(mut self, rows: Vec<Vec<crate::markup::InlineButton>>) -> Self {
        self.reply_markup = Some(Box::new(crate::markup::inline_keyboard(rows)));
        self
    }

    /// Builds the `messages.sendMessage` method.
    #[must_use]
    pub fn build(self) -> Method {
        Method::MessagesSendMessage(Box::new(MessagesSendMessage {
            flags: 0,
            no_webpage: self.no_webpage,
            silent: self.silent,
            background: self.background,
            clear_draft: self.clear_draft,
            noforwards: self.noforwards,
            update_stickersets_order: false,
            invert_media: self.invert_media,
            allow_paid_floodskip: false,
            peer: Box::new(self.peer),
            reply_to: None,
            message: self.message,
            random_id: random_id(),
            reply_markup: self.reply_markup,
            entities: self.entities,
            schedule_date: None,
            schedule_repeat_period: None,
            send_as: None,
            quick_reply_shortcut: None,
            effect: None,
            allow_paid_stars: None,
            suggested_post: None,
            rich_message: None,
        }))
    }
}

/// Builds a `messages.sendMedia` request for `media`.
#[must_use]
pub fn build_send_media(peer: InputPeer, media: InputMedia, caption: &str) -> Method {
    Method::MessagesSendMedia(Box::new(MessagesSendMedia {
        flags: 0,
        silent: false,
        background: false,
        clear_draft: false,
        noforwards: false,
        update_stickersets_order: false,
        invert_media: false,
        allow_paid_floodskip: false,
        peer: Box::new(peer),
        reply_to: None,
        media: Box::new(media),
        message: caption.to_owned(),
        random_id: random_id(),
        reply_markup: None,
        entities: None,
        schedule_date: None,
        schedule_repeat_period: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: None,
        allow_paid_stars: None,
        suggested_post: None,
    }))
}

/// Sends an uploaded photo with `caption`.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_photo<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    file: InputFile,
) -> Result<Object> {
    let media = InputMedia::InputMediaUploadedPhoto {
        flags: 0,
        spoiler: false,
        live_photo: false,
        file: Box::new(file),
        stickers: None,
        ttl_seconds: None,
        video: None,
    };
    client.invoke(build_send_media(peer, media, caption))
}

/// Sends an uploaded document with `caption`.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_document<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    file: InputFile,
    mime_type: &str,
    attributes: Vec<DocumentAttribute>,
) -> Result<Object> {
    let media = InputMedia::InputMediaUploadedDocument {
        flags: 0,
        nosound_video: false,
        force_file: false,
        spoiler: false,
        file: Box::new(file),
        thumb: None,
        mime_type: mime_type.to_owned(),
        attributes,
        stickers: None,
        video_cover: None,
        video_timestamp: None,
        ttl_seconds: None,
    };
    client.invoke(build_send_media(peer, media, caption))
}

/// Sends `builder`'s message and returns the response object.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send<T: Transport>(client: &mut Client<T>, builder: RequestBuilder) -> Result<Object> {
    client.invoke(builder.build())
}

/// Returns a random non-zero message identifier.
fn random_id() -> i64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&rtd_crypto::random_bytes(8));
    let id = i64::from_le_bytes(bytes);
    if id == 0 { 1 } else { id }
}

/// Returns an error when a send answer is not `Updates`.
#[must_use]
pub fn unexpected_response(object: &Object) -> Error {
    Error::Unexpected(format!(
        "messages.sendMessage returned unexpected constructor {:#010x}",
        object.type_id()
    ))
}

#[cfg(test)]
mod tests {
    use rtd_tg::{InputPeer, Method};

    use super::RequestBuilder;

    #[test]
    fn builds_send_media() {
        use rtd_tg::{InputFile, InputMedia, InputPeer};
        let media = InputMedia::InputMediaUploadedPhoto {
            flags: 0,
            spoiler: false,
            live_photo: false,
            file: Box::new(InputFile::InputFile {
                id: 1,
                parts: 1,
                name: "x".to_owned(),
                md5_checksum: String::new(),
            }),
            stickers: None,
            ttl_seconds: None,
            video: None,
        };
        let method = super::build_send_media(InputPeer::InputPeerEmpty, media, "cap");
        assert!(matches!(method, rtd_tg::Method::MessagesSendMedia(_)));
    }

    #[test]
    fn html_builder_parses_entities() {
        let method = RequestBuilder::html(InputPeer::InputPeerEmpty, "<b>hi</b>").build();
        match method {
            Method::MessagesSendMessage(request) => {
                assert_eq!(request.message, "hi");
                let entities = request.entities.expect("entities");
                assert_eq!(entities.len(), 1);
            }
            _ => panic!("expected messages.sendMessage"),
        }
    }

    #[test]
    fn builder_sets_options() {
        let method = RequestBuilder::new(InputPeer::InputPeerEmpty, "hi")
            .silent()
            .no_webpage()
            .build();
        match method {
            Method::MessagesSendMessage(request) => {
                assert!(request.silent);
                assert!(request.no_webpage);
                assert!(!request.background);
                assert_eq!(request.message, "hi");
                assert_ne!(request.random_id, 0);
            }
            _ => panic!("expected messages.sendMessage"),
        }
    }
}
