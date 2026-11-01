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
#![warn(missing_docs)]

pub mod grpc;

pub use grpc::GrpcCore;
