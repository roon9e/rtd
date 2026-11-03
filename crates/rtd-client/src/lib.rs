//! MTProto 2.0 client runtime.
//!
//! Composes the workspace's sans-IO protocol crates into a client: a framed
//! transport, the requester authorization-key exchange, an encrypted session,
//! and RPC invocation at the canonical layer.
#![warn(missing_docs)]

pub mod async_client;
pub mod auth;
pub mod client;
pub mod deeplink;
pub mod error;
pub mod middleware;
pub mod qr;
pub mod session;
pub mod storage;
pub mod takeout;
pub mod transport;
pub mod ws;

pub use async_client::AsyncClient;
pub use auth::{ConstantPassword, PasswordProvider};
pub use client::{Client, InitParams};
pub use deeplink::{Deeplink, DeeplinkError};
pub use error::{Error, Result};
pub use middleware::{Delay, FloodWait, Middleware, RateLimit, ThreadDelay};
pub use qr::{LoginToken, login_url};
pub use session::{ClientSession, SaltStore, SessionData};
pub use storage::{FileStorage, MemoryStorage, Storage};
pub use takeout::TakeoutConfig;
pub use transport::{
    DuplexStream, Framed, ReadWrite, TcpTransport, Transport, connect_tcp, dial_tcp,
};
pub use ws::{WebSocketIo, connect_websocket};
