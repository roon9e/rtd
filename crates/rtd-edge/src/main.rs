//! The rtd sidecar edge service.
//!
//! This is the service binary for the gRPC edge boundary defined by
//! [`rtd_edge_proto`]. The protocol edge and its wiring are implemented in the
//! server stage; today the binary exposes the contract version and can validate
//! a core's declared version, which is what the boundary's handshake checks.
#![warn(missing_docs)]

use rtd_edge_proto::{CONTRACT_VERSION, HelloRequest, handshake};

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => {
            println!("rtd-edge contract version {CONTRACT_VERSION}");
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
            let request = HelloRequest {
                contract_version: version,
                core_id: "cli".to_owned(),
            };
            match handshake(&request) {
                Ok(response) => println!("accepted contract version {}", response.contract_version),
                Err(message) => {
                    eprintln!("rejected: {message}");
                    std::process::exit(1);
                }
            }
        }
        Some(other) => {
            eprintln!("unknown argument {other:?}");
            std::process::exit(2);
        }
    }
}
