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
pub mod json;
pub mod keepalive;
pub mod markup;
pub mod md5;
pub mod message;
pub mod middleware;
pub mod pagination;
pub mod peers;
pub mod pool;
pub mod qr;
pub mod session;
pub mod storage;
pub mod takeout;
pub mod thumbnail;
pub mod transport;
pub mod updates;
pub mod uploader;
pub mod ws;

pub use async_client::AsyncClient;
pub use auth::{ConstantPassword, PasswordProvider};
pub use calls::{CallControl, CallError, CallState};
pub use client::{Client, InitParams};
pub use dcs::{DataCenter, Endpoint, Protocol, Resolver, StaticResolver};
pub use deeplink::{Deeplink, DeeplinkError};
pub use downloader::Downloader;
pub use entity::{parse_html, parse_markdown};
pub use error::{Error, Result};
pub use fileid::{FileId, Type as FileType};
pub use json::{JsonValue, describe, type_name};
pub use keepalive::Keepalive;
pub use markup::{InlineButton, inline_keyboard};
pub use message::RequestBuilder;
pub use middleware::{Delay, FloodWait, Middleware, RateLimit, ThreadDelay};
pub use pagination::{
    BlockedIter, DialogsIter, HistoryIter, ParticipantsIter, PhotosIter, fetch_dialogs,
    fetch_history, fetch_participants,
};
pub use peers::{PeerManager, ResolvedPeer};
pub use pool::Pool;
pub use qr::{LoginToken, login_url};
pub use session::{ClientSession, SaltStore, SessionData};
pub use storage::{FileStorage, ImportedSession, MemoryStorage, Storage, decode_telethon};
pub use takeout::TakeoutConfig;
pub use thumbnail::{ThumbnailError, expand as expand_thumbnail};
pub use transport::{
    DuplexReader, DuplexStream, DuplexWriter, Framed, ReadTransport, ReadWrite, TcpTransport,
    Transport, WriteTransport, connect_tcp, dial_tcp,
};
pub use updates::{
    Difference, PtsUpdate, PushResult, SequenceBox, fetch_difference, pts_update, qts_update,
    sort_by_pts,
};
pub use uploader::{NoProgress, Progress, Uploader};
pub use ws::{WebSocketIo, connect_websocket};
