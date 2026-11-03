//! The rtd MTProto edge library.
//!
//! This crate is the sidecar service that fronts a core implementation over the
//! `telesrv.coreexec.v1.CoreExecService` gRPC contract defined by
//! [`rtd_edge_proto`]. The runnable service binary is built on top of the
//! library API here.
//!
//! The edge reaches durable state and method execution through the
//! [`rtd_server::core::Core`] abstraction. Two implementations satisfy it: the
//! in-process [`rtd_server::core::LocalCore`] and the remote [`GrpcCore`]
//! adapter in [`grpc`], which dials a core's `CoreExecService`. The edge runs
//! with either without changing observable protocol behavior.
//!
//! - [`config`] — the command-line and environment configuration.
//! - [`error`] — the errors reported while constructing or running the edge.
//! - [`exchange`] — the server-side key exchange supplied to the actor.
//! - [`service`] — assembling and running the edge from configuration.
//! - [`grpc`] — the tonic adapter to a core's `CoreExecService`.
#![warn(missing_docs)]

pub mod config;
pub mod config_file;
pub mod delivery;
pub mod error;
pub mod exchange;
pub mod grpc;
pub mod identity;
pub mod location;
pub mod projection;
pub mod service;

pub use config::EdgeConfig;
pub use config_file::{parse as parse_config, to_args as config_to_args};
pub use delivery::{Delivery, Envelope, LocalDelivery};
pub use error::EdgeError;
pub use exchange::ServerExchange;
pub use grpc::GrpcCore;
pub use identity::{BuildInfo, build_info};
pub use location::{EdgeLocation, Location, LocationProvider, LocationRegistry, StaticProvider};
pub use projection::{Projection, Watermark};
