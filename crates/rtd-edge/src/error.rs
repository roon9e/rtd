//! Errors constructing or running the edge service.
//!
//! The binary reports these on standard error and exits non-zero; a
//! [`EdgeError::Usage`] additionally prints the usage text.

use std::fmt;
use std::io;
use std::path::PathBuf;

use rtd_server::ingress::IngressError;

/// An error constructing or running the edge service.
#[derive(Debug)]
#[non_exhaustive]
pub enum EdgeError {
    /// A command-line or environment argument was invalid.
    Usage(String),
    /// The configured RSA private key file could not be read.
    KeyFile {
        /// The path that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// The RSA private key PEM could not be parsed.
    KeyParse(rtd_crypto::Error),
    /// The configured core endpoint could not be parsed or dialed.
    Core(tonic::transport::Error),
    /// The edge listener could not be bound or accept a connection.
    Ingress(IngressError),
}

impl fmt::Display for EdgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => f.write_str(message),
            Self::KeyFile { path, source } => {
                write!(f, "could not read RSA key {}: {source}", path.display())
            }
            Self::KeyParse(error) => write!(f, "could not parse RSA key: {error}"),
            Self::Core(error) => write!(f, "core endpoint failed: {error}"),
            Self::Ingress(error) => write!(f, "listener failed: {error}"),
        }
    }
}

impl std::error::Error for EdgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Usage(_) => None,
            Self::KeyFile { source, .. } => Some(source),
            Self::KeyParse(error) => Some(error),
            Self::Core(error) => Some(error),
            Self::Ingress(error) => Some(error),
        }
    }
}
