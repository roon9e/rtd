//! MTProto server edge, RPC dispatch and multi-DC runtime.
//!
//! This is the runtime layer. It owns tokio sockets, timers and (in the `rtd-edge`
//! binary) the gRPC boundary; the protocol crates own none of that. Modules:
//!
//! - [`ingress`] — inbound transport detection and unwrapping.
//! - [`session`] — the transient session registry and auth-key cache.
//! - [`rpc`] — invocation-wrapper handling, per-session layer binding and dispatch.
//! - [`core`] — the core-boundary trait and its in-process implementation.
#![warn(missing_docs)]

pub mod core;
pub mod ingress;
pub mod rpc;
pub mod session;
