//! MTProto server edge, RPC dispatch and multi-DC runtime.
//!
//! This is the runtime layer. It owns tokio sockets, timers and (in the `rtd-edge`
//! binary) the gRPC boundary; the protocol crates own none of that. Modules:
//!
//! - [`ingress`] — inbound transport detection and unwrapping.
//! - [`session`] — the transient session registry and auth-key cache.
//! - [`actor`] — the per-connection actor, service messages and admission budgets.
//! - [`rpc`] — invocation-wrapper handling, per-session layer binding and dispatch.
//! - [`ratelimit`] — per-method rate limiting and `FLOOD_WAIT` emission.
//! - [`core`] — the core-boundary trait and its in-process implementation.
#![warn(missing_docs)]

pub mod actor;
pub mod core;
pub mod ingress;
pub mod metrics;
pub mod ratelimit;
pub mod rpc;
pub mod session;

pub use metrics::Metrics;
pub use ratelimit::{ClassLimit, MethodClass, RateLimitConfig, RateLimiter};
