//! Offline Telegram seed fetchers built on the [`rtd`] client library.
//!
//! These binaries are the Rust port of the Go `telesrv` fetchers
//! (`cmd/{appearancefetch,catalogfetch,langpackfetch,stickerfetch,giftfetch}`).
//! They talk to official Telegram with the public TDesktop application
//! credentials, snapshot catalog/reference data, and write it in the exact seed
//! formats `telesrv` consumes. The port is read-only: it never imports data back
//! into a server.
//!
//! The library half is shared plumbing: an official-Telegram client bootstrap
//! ([`client`]), credential and session helpers ([`creds`], [`client::Tg`]),
//! Go-compatible JSON and string writers ([`gojson`], [`goquote`]) so artifacts
//! can be diffed byte-for-byte against the Go tools, and the per-tool seed value
//! models under [`seed`].
//!
//! [`rtd`]: https://github.com/roon9e/rtd
#![warn(missing_docs)]

pub mod client;
pub mod creds;
pub mod fsutil;
pub mod gojson;
pub mod goquote;
pub mod proxy;
pub mod seed;
pub mod timefmt;

/// The crate-wide boxed error type.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// The crate-wide result alias.
pub type Result<T> = std::result::Result<T, Error>;
