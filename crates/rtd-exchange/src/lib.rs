//! MTProto authorization-key exchange flows.
//!
//! This crate implements both sides of the MTProto 2.0 auth-key exchange as
//! sans-IO state machines: the caller supplies a [`Stream`] that moves plaintext
//! frames, and time and randomness are injected. The client flow is
//! [`client_exchange`]; the server flow is [`server_exchange`].
#![warn(missing_docs)]

pub mod client;
pub mod error;
pub mod rng;
pub mod server;
pub mod stream;
mod util;

pub use client::{ClientConfig, ClientExchangeResult, client_exchange};
pub use error::{Error, Result};
pub use rng::{ServerRng, TestServerRng};
pub use server::{ServerConfig, ServerExchangeResult, server_exchange};
pub use stream::Stream;
