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
    let args = apply_config(args)?;
    let config = EdgeConfig::parse(args, |name| std::env::var(name).ok())?;
    service::run(config).await
}

/// Expands a `--config <path>` file into flags prepended to `args`.
///
/// Explicit command-line flags therefore override the file. The `--config`
/// option itself is removed before the remaining arguments are parsed.
fn apply_config(args: Vec<String>) -> Result<Vec<String>, EdgeError> {
    let mut filtered = Vec::new();
    let mut config_path = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--config" {
            config_path = iter.next();
        } else if let Some(value) = arg.strip_prefix("--config=") {
            config_path = Some(value.to_owned());
        } else {
            filtered.push(arg);
        }
    }
    let Some(path) = config_path else {
        return Ok(filtered);
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|source| EdgeError::Usage(format!("cannot read config {path}: {source}")))?;
    let mut extra = rtd_edge::config_file::to_args(&rtd_edge::config_file::parse(&text), &|name| {
        std::env::var(name).ok()
    });
    extra.extend(filtered);
    Ok(extra)
}
