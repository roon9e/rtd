//! MTProto 2.0 client runtime.
//!
//! Composes the workspace's sans-IO protocol crates into a client: a framed
//! transport, the requester authorization-key exchange, an encrypted session,
//! and RPC invocation at the canonical layer.
#![warn(missing_docs)]

pub mod async_client;
pub mod auth;
pub mod calls;
pub mod client;
pub mod dcs;
pub mod deeplink;
pub mod downloader;
pub mod entity;
pub mod error;
pub mod fileid;
pub mod inline;
pub mod json;
pub mod keepalive;
pub mod markup;
pub mod md5;
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
pub use auth::{ConstantPassword, PasswordProvider};
pub use calls::{
    CallControl, CallError, CallState, DhConfig, accept_call, confirm_call, default_protocol,
    fetch_dh_config, g_a_hash, request_call,
};
pub use client::{Client, InitParams};
pub use dcs::{
    DataCenter, DnsConfig, DnsError, Endpoint, Protocol, Resolver, StaticResolver, migrate_target,
    parse_dns_config,
};
pub use deeplink::{Deeplink, DeeplinkError};
pub use downloader::Downloader;
pub use entity::{parse_html, parse_markdown};
pub use error::{Error, Result};
pub use fileid::{FileId, PhotoSizeSource, Type as FileType};
pub use inline::{article, set_inline_bot_results};
pub use json::{JsonValue, describe, type_name};
pub use keepalive::{Backoff, Keepalive};
pub use markup::{InlineButton, inline_keyboard};
pub use members::{ChannelMembers, Member, MemberStatus, fetch_members};
pub use message::{
    RequestBuilder, build_send_media, delete_messages, edit_message, edit_message_html,
    edit_message_with, forward_messages, send_document, send_photo,
};
pub use middleware::{Delay, FloodWait, Middleware, RateLimit, ThreadDelay, Trace, Tracer};
pub use pagination::{
    BlockedIter, DialogsIter, Hasher, HistoryIter, ParticipantsIter, PhotosIter, Query, fetch_all,
    fetch_dialogs, fetch_history, fetch_participants, for_each,
};
pub use peers::{
    MemoryPeerStorage, PeerKey, PeerKind, PeerManager, PeerStorage, PeerValue, ResolvedPeer,
};
pub use pool::{Pool, SelectedKey, SessionOptions};
pub use qr::{LoginToken, login_url};
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
    ChannelDifference, Difference, MemoryStateStorage, PtsUpdate, PushResult, SequenceBox, State,
    StateStorage, fetch_channel_difference, fetch_difference, pts_update, qts_update, sort_by_pts,
};
pub use uploader::{NoProgress, Progress, Source, Uploader};
pub use ws::{WebSocketIo, connect_websocket};
