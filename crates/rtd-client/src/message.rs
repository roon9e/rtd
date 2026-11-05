//! Message builders.
//!
//! [`RequestBuilder`] constructs a `messages.sendMessage` request from a peer,
//! text and options; [`send`] invokes it through a client.

use rtd_tg::{
    ChannelsJoinChannel, DocumentAttribute, InputChannel, InputDocument, InputFile, InputGame,
    InputGeoPoint, InputMedia, InputPeer, InputReplyTo, InputSingleMedia, InputUser, MessageEntity,
    MessagesDeleteMessages, MessagesEditMessage, MessagesForwardMessages, MessagesReport,
    MessagesSaveDraft, MessagesSendMedia, MessagesSendMessage, MessagesSendMultiMedia,
    MessagesSetTyping, Method, Object, Poll, PollAnswer, ReplyMarkup, SendMessageAction,
    TextWithEntities,
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
    reply_to: Option<Box<InputReplyTo>>,
    schedule_date: Option<i32>,
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
            reply_to: None,
            schedule_date: None,
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

    /// Replies to a specific message.
    #[must_use]
    pub fn reply_to(mut self, reply_to: InputReplyTo) -> Self {
        self.reply_to = Some(Box::new(reply_to));
        self
    }

    /// Schedules the message for `date` (a Unix timestamp).
    #[must_use]
    pub fn schedule_date(mut self, date: i32) -> Self {
        self.schedule_date = Some(date);
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
            reply_to: self.reply_to,
            message: self.message,
            random_id: random_id(),
            reply_markup: self.reply_markup,
            entities: self.entities,
            schedule_date: self.schedule_date,
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
    client.invoke(build_send_media(
        peer,
        document_media(file, mime_type, attributes),
        caption,
    ))
}

/// Builds an uploaded-document media value.
#[must_use]
pub fn document_media(
    file: InputFile,
    mime_type: &str,
    attributes: Vec<DocumentAttribute>,
) -> InputMedia {
    InputMedia::InputMediaUploadedDocument {
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
    }
}

/// Sends already-uploaded `media` with `caption` (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_media<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    media: InputMedia,
) -> Result<Object> {
    client.invoke(build_send_media(peer, media, caption))
}

/// Builds a `messages.sendMultiMedia` (album) request from `(media, caption)`
/// pairs.
#[must_use]
pub fn build_send_media_group(peer: InputPeer, items: Vec<(InputMedia, String)>) -> Method {
    let multi_media = items
        .into_iter()
        .map(|(media, caption)| InputSingleMedia {
            flags: 0,
            media: Box::new(media),
            random_id: random_id(),
            message: caption,
            entities: None,
        })
        .collect();
    Method::MessagesSendMultiMedia(Box::new(MessagesSendMultiMedia {
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
        multi_media,
        schedule_date: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: None,
        allow_paid_stars: None,
    }))
}

/// Sends an album (`messages.sendMultiMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_media_group<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    items: Vec<(InputMedia, String)>,
) -> Result<Object> {
    client.invoke(build_send_media_group(peer, items))
}

/// Sends a geographic point (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_geo<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    lat: f64,
    long: f64,
    accuracy_radius: Option<i32>,
) -> Result<Object> {
    let media = InputMedia::InputMediaGeoPoint {
        geo_point: Box::new(InputGeoPoint::InputGeoPoint {
            flags: if accuracy_radius.is_some() { 1 } else { 0 },
            lat,
            long,
            accuracy_radius,
        }),
    };
    client.invoke(build_send_media(peer, media, caption))
}

/// Sends a contact (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_contact<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    phone_number: &str,
    first_name: &str,
    last_name: &str,
    vcard: &str,
) -> Result<Object> {
    let media = InputMedia::InputMediaContact {
        phone_number: phone_number.to_owned(),
        first_name: first_name.to_owned(),
        last_name: last_name.to_owned(),
        vcard: vcard.to_owned(),
    };
    client.invoke(build_send_media(peer, media, caption))
}

/// The parameters of a new poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollSpec {
    /// The poll question.
    pub question: String,
    /// The answer options.
    pub answers: Vec<String>,
    /// Whether voters are public.
    pub public_voters: bool,
    /// Whether multiple answers may be chosen.
    pub multiple_choice: bool,
    /// Whether this is a quiz.
    pub quiz: bool,
}

impl PollSpec {
    /// Creates a poll spec.
    #[must_use]
    pub fn new(question: impl Into<String>, answers: Vec<String>) -> Self {
        Self {
            question: question.into(),
            answers,
            public_voters: false,
            multiple_choice: false,
            quiz: false,
        }
    }

    /// Enables public voters.
    #[must_use]
    pub fn public(mut self) -> Self {
        self.public_voters = true;
        self
    }

    /// Enables multiple choice.
    #[must_use]
    pub fn multiple_choice(mut self) -> Self {
        self.multiple_choice = true;
        self
    }

    /// Marks the poll a quiz.
    #[must_use]
    pub fn quiz(mut self) -> Self {
        self.quiz = true;
        self
    }
}

/// Sends a poll (`messages.sendMedia` with an `inputMediaPoll`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_poll<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    spec: PollSpec,
) -> Result<Object> {
    let poll = Poll {
        id: 0,
        flags: 0,
        closed: false,
        public_voters: spec.public_voters,
        multiple_choice: spec.multiple_choice,
        quiz: spec.quiz,
        open_answers: false,
        revoting_disabled: false,
        shuffle_answers: false,
        hide_results_until_close: false,
        creator: false,
        subscribers_only: false,
        question: Box::new(TextWithEntities {
            text: spec.question,
            entities: Vec::new(),
        }),
        answers: spec
            .answers
            .into_iter()
            .map(|text| PollAnswer::InputPollAnswer {
                flags: 0,
                text: Box::new(TextWithEntities {
                    text,
                    entities: Vec::new(),
                }),
                media: None,
            })
            .collect(),
        close_period: None,
        close_date: None,
        countries_iso2: None,
        hash: 0,
    };
    let media = InputMedia::InputMediaPoll {
        flags: 0,
        poll: Box::new(poll),
        correct_answers: None,
        attached_media: None,
        solution: None,
        solution_entities: None,
        solution_media: None,
    };
    client.invoke(build_send_media(peer, media, caption))
}

/// Parameters for [`audio_media`] and [`send_audio`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSpec {
    /// The audio MIME type.
    pub mime_type: String,
    /// The duration in seconds.
    pub duration: i32,
    /// The track title.
    pub title: Option<String>,
    /// The performer.
    pub performer: Option<String>,
}

/// Parameters for [`video_media`] and [`send_video`].
#[derive(Debug, Clone, PartialEq)]
pub struct VideoSpec {
    /// The video MIME type.
    pub mime_type: String,
    /// The frame width in pixels.
    pub width: i32,
    /// The frame height in pixels.
    pub height: i32,
    /// The duration in seconds.
    pub duration: i32,
    /// Whether the video supports streaming.
    pub supports_streaming: bool,
    /// Whether this is a round video message.
    pub round_message: bool,
    /// Whether the video has no sound.
    pub nosound: bool,
}

/// Parameters for [`venue_media`] and [`send_venue`].
#[derive(Debug, Clone)]
pub struct VenueSpec {
    /// The venue location.
    pub geo_point: InputGeoPoint,
    /// The venue name.
    pub title: String,
    /// The street address.
    pub address: String,
    /// The provider name.
    pub provider: String,
    /// The provider's venue id.
    pub venue_id: String,
    /// The venue type.
    pub venue_type: String,
}

/// Builds an uploaded audio media value.
#[must_use]
pub fn audio_media(file: InputFile, spec: AudioSpec) -> InputMedia {
    document_media(
        file,
        &spec.mime_type,
        vec![DocumentAttribute::DocumentAttributeAudio {
            flags: 0,
            voice: false,
            duration: spec.duration,
            title: spec.title,
            performer: spec.performer,
            waveform: None,
        }],
    )
}

/// Builds an uploaded video media value.
#[must_use]
pub fn video_media(file: InputFile, spec: VideoSpec) -> InputMedia {
    document_media(
        file,
        &spec.mime_type,
        vec![DocumentAttribute::DocumentAttributeVideo {
            flags: 0,
            round_message: spec.round_message,
            supports_streaming: spec.supports_streaming,
            nosound: spec.nosound,
            duration: f64::from(spec.duration),
            w: spec.width,
            h: spec.height,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }],
    )
}

/// Builds an uploaded animation (GIF) media value.
#[must_use]
pub fn gif_media(file: InputFile, mime_type: &str, width: i32, height: i32) -> InputMedia {
    document_media(
        file,
        mime_type,
        vec![
            DocumentAttribute::DocumentAttributeAnimated,
            DocumentAttribute::DocumentAttributeImageSize {
                w: width,
                h: height,
            },
        ],
    )
}

/// Builds a dice media value.
#[must_use]
pub fn dice_media(emoticon: &str) -> InputMedia {
    InputMedia::InputMediaDice {
        emoticon: emoticon.to_owned(),
    }
}

/// Builds a venue media value.
#[must_use]
pub fn venue_media(spec: VenueSpec) -> InputMedia {
    InputMedia::InputMediaVenue {
        geo_point: Box::new(spec.geo_point),
        title: spec.title,
        address: spec.address,
        provider: spec.provider,
        venue_id: spec.venue_id,
        venue_type: spec.venue_type,
    }
}

/// Builds a game media value from a game short name.
#[must_use]
pub fn game_media(bot_id: InputUser, short_name: &str) -> InputMedia {
    InputMedia::InputMediaGame {
        id: Box::new(InputGame::InputGameShortName {
            bot_id: Box::new(bot_id),
            short_name: short_name.to_owned(),
        }),
    }
}

/// Builds a web-page preview media value.
#[must_use]
pub fn webpage_media(
    url: &str,
    force_large_media: bool,
    force_small_media: bool,
    optional: bool,
) -> InputMedia {
    InputMedia::InputMediaWebPage {
        flags: 0,
        force_large_media,
        force_small_media,
        optional,
        url: url.to_owned(),
    }
}

/// Sends an uploaded audio file with `caption` (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_audio<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    file: InputFile,
    spec: AudioSpec,
) -> Result<Object> {
    client.invoke(build_send_media(peer, audio_media(file, spec), caption))
}

/// Sends an uploaded video with `caption` (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_video<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    file: InputFile,
    spec: VideoSpec,
) -> Result<Object> {
    client.invoke(build_send_media(peer, video_media(file, spec), caption))
}

/// Sends an uploaded animation (GIF) with `caption` (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_gif<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    file: InputFile,
    mime_type: &str,
    width: i32,
    height: i32,
) -> Result<Object> {
    client.invoke(build_send_media(
        peer,
        gif_media(file, mime_type, width, height),
        caption,
    ))
}

/// Sends a dice with `emoticon` (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_dice<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    emoticon: &str,
) -> Result<Object> {
    client.invoke(build_send_media(peer, dice_media(emoticon), ""))
}

/// Sends a venue with `caption` (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_venue<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    spec: VenueSpec,
) -> Result<Object> {
    client.invoke(build_send_media(peer, venue_media(spec), caption))
}

/// Sends a game with `caption` (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_game<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    bot_id: InputUser,
    short_name: &str,
) -> Result<Object> {
    client.invoke(build_send_media(
        peer,
        game_media(bot_id, short_name),
        caption,
    ))
}

/// Sends a web-page preview (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_webpage<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    caption: &str,
    url: &str,
    force_large_media: bool,
    force_small_media: bool,
    optional: bool,
) -> Result<Object> {
    let media = webpage_media(url, force_large_media, force_small_media, optional);
    client.invoke(build_send_media(peer, media, caption))
}

/// Builds an already-uploaded sticker media value.
#[must_use]
pub fn sticker_media(document: InputDocument, spoiler: bool) -> InputMedia {
    InputMedia::InputMediaDocument {
        flags: 0,
        spoiler,
        id: Box::new(document),
        video_cover: None,
        video_timestamp: None,
        ttl_seconds: None,
        query: None,
    }
}

/// Sends an already-uploaded sticker (`messages.sendMedia`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn send_sticker<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    document: InputDocument,
) -> Result<Object> {
    client.invoke(build_send_media(peer, sticker_media(document, false), ""))
}

/// Sets the typing action for `peer` (`messages.setTyping`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn set_typing<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    action: SendMessageAction,
) -> Result<Object> {
    let query = Method::MessagesSetTyping(Box::new(MessagesSetTyping {
        flags: 0,
        peer: Box::new(peer),
        top_msg_id: None,
        action: Box::new(action),
    }));
    client.invoke(query)
}

/// Saves a text draft for `peer` (`messages.saveDraft`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn save_draft<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    text: &str,
) -> Result<Object> {
    let query = Method::MessagesSaveDraft(Box::new(MessagesSaveDraft {
        flags: 0,
        no_webpage: false,
        invert_media: false,
        reply_to: None,
        peer: Box::new(peer),
        message: text.to_owned(),
        entities: None,
        media: None,
        effect: None,
        suggested_post: None,
        rich_message: None,
    }));
    client.invoke(query)
}

/// Reports messages `ids` in `peer` (`messages.report`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn report<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    ids: Vec<i32>,
    option: Vec<u8>,
    message: &str,
) -> Result<Object> {
    let query = Method::MessagesReport(Box::new(MessagesReport {
        peer: Box::new(peer),
        id: ids,
        option,
        message: message.to_owned(),
    }));
    client.invoke(query)
}

/// Joins a channel (`channels.joinChannel`).
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn join_channel<T: Transport>(client: &mut Client<T>, channel: InputChannel) -> Result<Object> {
    let query = Method::ChannelsJoinChannel(Box::new(ChannelsJoinChannel {
        channel: Box::new(channel),
    }));
    client.invoke(query)
}

/// Edits message `id` in `peer` to the given `text` and entities.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn edit_message_with<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    id: i32,
    text: String,
    entities: Option<Vec<MessageEntity>>,
) -> Result<Object> {
    let query = Method::MessagesEditMessage(Box::new(MessagesEditMessage {
        flags: 0,
        no_webpage: false,
        invert_media: false,
        peer: Box::new(peer),
        id,
        message: Some(text),
        media: None,
        reply_markup: None,
        entities,
        schedule_date: None,
        schedule_repeat_period: None,
        quick_reply_shortcut_id: None,
        rich_message: None,
    }));
    client.invoke(query)
}

/// Edits message `id` in `peer` to plain `text`.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn edit_message<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    id: i32,
    text: &str,
) -> Result<Object> {
    edit_message_with(client, peer, id, text.to_owned(), None)
}

/// Edits message `id` in `peer` to HTML `text`, parsing its entities.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn edit_message_html<T: Transport>(
    client: &mut Client<T>,
    peer: InputPeer,
    id: i32,
    html: &str,
) -> Result<Object> {
    let (text, entities) = crate::entity::parse_html(html);
    let entities = (!entities.is_empty()).then_some(entities);
    edit_message_with(client, peer, id, text, entities)
}

/// Forwards messages `ids` from `from` to `to`.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn forward_messages<T: Transport>(
    client: &mut Client<T>,
    from: InputPeer,
    to: InputPeer,
    ids: Vec<i32>,
) -> Result<Object> {
    let random_ids: Vec<i64> = ids.iter().map(|_| random_id()).collect();
    let query = Method::MessagesForwardMessages(Box::new(MessagesForwardMessages {
        flags: 0,
        silent: false,
        background: false,
        with_my_score: false,
        drop_author: false,
        drop_media_captions: false,
        noforwards: false,
        allow_paid_floodskip: false,
        from_ephemeral: false,
        from_peer: Box::new(from),
        id: ids,
        random_id: random_ids,
        to_peer: Box::new(to),
        top_msg_id: None,
        reply_to: None,
        schedule_date: None,
        schedule_repeat_period: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: None,
        video_timestamp: None,
        allow_paid_stars: None,
        suggested_post: None,
    }));
    client.invoke(query)
}

/// Deletes messages `ids`, optionally for everyone when `revoke` is set.
///
/// # Errors
///
/// Returns a transport, crypto, decode or RPC error.
pub fn delete_messages<T: Transport>(
    client: &mut Client<T>,
    ids: Vec<i32>,
    revoke: bool,
) -> Result<Object> {
    let query = Method::MessagesDeleteMessages(Box::new(MessagesDeleteMessages {
        flags: if revoke { 1 } else { 0 },
        revoke,
        id: ids,
    }));
    client.invoke(query)
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

    #[test]
    fn builds_a_media_group() {
        use rtd_tg::{InputGeoPoint, InputMedia};

        let geo = InputMedia::InputMediaGeoPoint {
            geo_point: Box::new(InputGeoPoint::InputGeoPoint {
                flags: 0,
                lat: 1.0,
                long: 2.0,
                accuracy_radius: None,
            }),
        };
        let method = super::build_send_media_group(
            InputPeer::InputPeerEmpty,
            vec![(geo.clone(), "a".to_owned()), (geo, "b".to_owned())],
        );
        match method {
            Method::MessagesSendMultiMedia(request) => {
                assert_eq!(request.multi_media.len(), 2);
                assert_eq!(request.multi_media[0].message, "a");
                assert_ne!(request.multi_media[0].random_id, 0);
            }
            _ => panic!("expected messages.sendMultiMedia"),
        }
    }

    #[test]
    fn builds_staged_media() {
        use rtd_tg::{InputFile, InputGeoPoint, InputMedia};

        let file = InputFile::InputFile {
            id: 1,
            parts: 1,
            name: "x".to_owned(),
            md5_checksum: String::new(),
        };
        assert!(matches!(
            super::audio_media(
                file.clone(),
                super::AudioSpec {
                    mime_type: "audio/mpeg".to_owned(),
                    duration: 3,
                    title: Some("t".to_owned()),
                    performer: None,
                }
            ),
            InputMedia::InputMediaUploadedDocument { .. }
        ));
        assert!(matches!(
            super::video_media(
                file.clone(),
                super::VideoSpec {
                    mime_type: "video/mp4".to_owned(),
                    width: 16,
                    height: 16,
                    duration: 1,
                    supports_streaming: true,
                    round_message: false,
                    nosound: false,
                }
            ),
            InputMedia::InputMediaUploadedDocument { .. }
        ));
        assert!(matches!(
            super::gif_media(file, "image/gif", 8, 8),
            InputMedia::InputMediaUploadedDocument { .. }
        ));
        assert!(matches!(
            super::dice_media("\u{1F3B2}"),
            InputMedia::InputMediaDice { .. }
        ));
        assert!(matches!(
            super::venue_media(super::VenueSpec {
                geo_point: InputGeoPoint::InputGeoPoint {
                    flags: 0,
                    lat: 1.0,
                    long: 2.0,
                    accuracy_radius: None,
                },
                title: "t".to_owned(),
                address: "a".to_owned(),
                provider: "p".to_owned(),
                venue_id: "1".to_owned(),
                venue_type: "x".to_owned(),
            }),
            InputMedia::InputMediaVenue { .. }
        ));
        assert!(matches!(
            super::webpage_media("https://x", true, false, false),
            InputMedia::InputMediaWebPage { .. }
        ));
    }

    #[test]
    fn builder_carries_reply_and_schedule() {
        use rtd_tg::InputReplyTo;

        let method = RequestBuilder::new(InputPeer::InputPeerEmpty, "hi")
            .reply_to(InputReplyTo::InputReplyToEphemeralMessage { id: 5 })
            .schedule_date(1_700_000_000)
            .build();
        match method {
            Method::MessagesSendMessage(request) => {
                assert_eq!(request.schedule_date, Some(1_700_000_000));
                assert!(request.reply_to.is_some());
            }
            _ => panic!("expected messages.sendMessage"),
        }
    }

    #[test]
    fn builds_sticker_media() {
        use rtd_tg::{InputDocument, InputMedia};

        assert!(matches!(
            super::sticker_media(InputDocument::InputDocumentEmpty, false),
            InputMedia::InputMediaDocument { .. }
        ));
    }
}
