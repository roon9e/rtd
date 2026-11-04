//! MTProto 2.0 client runtime.
//!
//! Composes the workspace's sans-IO protocol crates into a client: a framed
//! transport, the requester authorization-key exchange, an encrypted session,
//! and RPC invocation at the canonical layer.
#![warn(missing_docs)]

pub mod async_client;
pub mod auth;
pub mod calls;
pub mod cdn;
pub mod client;
pub mod constant;
pub mod dcs;
pub mod deeplink;
pub mod downloader;
pub mod entity;
pub mod error;
pub mod fileid;
pub mod inline;
pub mod json;
pub mod keepalive;
pub mod manager;
pub mod markup;
pub mod md5;
#[cfg(feature = "media-webrtc")]
pub mod media;
pub mod members;
pub mod message;
pub mod middleware;
pub mod pagination;
pub mod peers;
pub mod pool;
pub mod qr;
pub mod session;
pub mod storage;
pub mod takeout;
pub mod tdesktop;
pub mod thumbnail;
pub mod transport;
pub mod unpack;
pub mod updates;
pub mod uploader;
pub mod ws;

pub use async_client::AsyncClient;
pub use auth::{BotToken, ConstantPassword, PasswordProvider};
pub use calls::{
    CallControl, CallError, CallState, DhConfig, GroupCall, MediaConnection, MediaEvents,
    SenderReport, TrackKind, accept_call, confirm_call, default_protocol, fetch_dh_config,
    g_a_hash, request_call,
};
pub use cdn::{
    CdnRedirect, CdnState, HashCache, HashWindow, VerifiedWindows, decrypt_in_place, verify_window,
};
pub use client::{Client, InitParams};
pub use constant::TdlibPeerId;
pub use dcs::{
    DataCenter, DnsConfig, DnsError, Endpoint, Migration, Protocol, Resolver, StaticResolver,
    migrate_plan, migrate_target, parse_dns_config,
};
pub use deeplink::{Deeplink, DeeplinkError};
pub use downloader::Downloader;
pub use entity::{parse_html, parse_markdown};
pub use error::{Error, Result};
pub use fileid::{FileId, PhotoSizeSource, Type as FileType, from_document, from_photo};
pub use inline::{article, set_inline_bot_results};
pub use json::{
    JsonError, JsonValue, base64_decode, base64_encode, decode_method, decode_object, describe,
    encode_method, encode_object, format as format_object, rpc_error_json, type_id, type_name,
};
pub use keepalive::{Backoff, Keepalive, Reconnect};
pub use manager::{DcRouter, ReconnectingClient};
pub use markup::{InlineButton, inline_keyboard};
#[cfg(feature = "media-webrtc")]
pub use media::WebRtcMedia;
pub use members::{ChannelMembers, Member, MemberStatus, fetch_members};
pub use message::{
    PollSpec, RequestBuilder, build_send_media, build_send_media_group, delete_messages,
    document_media, edit_message, edit_message_html, edit_message_with, forward_messages, send,
    send_contact, send_document, send_geo, send_media, send_media_group, send_photo, send_poll,
};
pub use middleware::{Delay, FloodWait, Middleware, RateLimit, ThreadDelay, Trace, Tracer};
pub use pagination::{
    BlockedIter, DialogsIter, Hasher, HistoryIter, ParticipantsIter, PhotosIter, Query, fetch_all,
    fetch_dialogs, fetch_history, fetch_participants, for_each,
};
pub use peers::{
    Channel, Chat, MemoryPeerStorage, PeerKey, PeerKind, PeerManager, PeerStorage, PeerValue,
    ResolvedPeer, Supergroup, User,
};
pub use pool::{Connection, Pool, Router, SelectedKey, SessionOptions};
pub use qr::{LoginToken, drive_login, login_url};
pub use session::{ClientSession, SaltStore, SessionData};
pub use storage::{FileStorage, ImportedSession, MemoryStorage, Storage, decode_telethon};
pub use takeout::TakeoutConfig;
pub use tdesktop::{Account as TdesktopAccount, TdesktopError, read as read_tdesktop};
pub use thumbnail::{ThumbnailError, expand as expand_thumbnail};
pub use transport::{
    DuplexReader, DuplexStream, DuplexWriter, Framed, ReadTransport, ReadWrite, Reader,
    TcpTransport, Transport, WriteTransport, connect_tcp, dial_tcp,
};
pub use unpack::{messages, messages_from_updates, update_message};
pub use updates::{
    Action, Callbacks, ChannelDifference, Difference, Manager, MemoryStateStorage, PtsUpdate,
    PushResult, SequenceBox, State, StateStorage, fetch_channel_difference, fetch_difference,
    pts_update, qts_update, sort_by_pts,
};
pub use uploader::{ConcurrentUploader, NoProgress, Progress, Source, Uploader};
pub use ws::{WebSocketIo, connect_websocket};
