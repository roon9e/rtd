//! The rtd sidecar edge service.
//!
//! This is the service binary for the gRPC boundary defined by
//! [`rtd_edge_proto`]. The protocol edge and its wiring are implemented in the
//! server stage; today the binary reports the contract's protocol version and can
//! validate a peer's declared version, which is what the boundary's handshake
//! checks.
#![warn(missing_docs)]

use rtd_edge_proto::{PROTOCOL_VERSION, version_is_supported};

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => {
            println!("rtd-edge protocol version {PROTOCOL_VERSION}");
        }
        Some("--check-version") => {
            let Some(value) = args.next() else {
                eprintln!("usage: rtd-edge --check-version <n>");
                std::process::exit(2);
            };
            let version: u32 = match value.parse() {
                Ok(version) => version,
                Err(error) => {
                    eprintln!("invalid version {value:?}: {error}");
                    std::process::exit(2);
                }
            };
            if version_is_supported(version) {
                println!("accepted protocol version {version}");
            } else {
                eprintln!(
                    "rejected: unsupported protocol version {version} (expected {PROTOCOL_VERSION})"
                );
                std::process::exit(1);
            }
        }
        Some(other) => {
            eprintln!("unknown argument {other:?}");
            std::process::exit(2);
        }
    }
}
