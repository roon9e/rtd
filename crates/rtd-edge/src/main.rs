//! The rtd sidecar edge service binary.
//!
//! The binary reads its configuration from the command line and environment
//! (see [`rtd_edge::config`]), builds the edge over either an in-process core or
//! the configured gRPC core endpoint, and serves until it receives a shutdown
//! signal.
#![warn(missing_docs)]

use std::process::ExitCode;

use rtd_edge::config::USAGE;
use rtd_edge::{EdgeConfig, EdgeError, service};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rtd-edge: {error}");
            if matches!(error, EdgeError::Usage(_)) {
                eprintln!();
                eprintln!("{USAGE}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Runs the binary, handling `--help`/`--version` before configuration.
async fn run() -> Result<(), EdgeError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("rtd-edge {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let config = EdgeConfig::parse(args, |name| std::env::var(name).ok())?;
    service::run(config).await
}
