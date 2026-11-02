//! MTProto 2.0 client runtime.
//!
//! Composes the workspace's sans-IO protocol crates into a client: a framed
//! transport, the requester authorization-key exchange, an encrypted session,
//! and RPC invocation at the canonical layer.
#![warn(missing_docs)]

pub mod async_client;
pub mod client;
pub mod error;
pub mod middleware;
pub mod session;
pub mod transport;

pub use async_client::AsyncClient;
pub use client::{Client, InitParams};
pub use error::{Error, Result};
pub use middleware::{Delay, FloodWait, Middleware, RateLimit, ThreadDelay};
pub use session::{ClientSession, SaltStore, SessionData};
pub use transport::{DuplexStream, Framed, ReadWrite, TcpTransport, Transport, connect_tcp};
