//! Errors produced by transport obfuscation and proxying.

use core::fmt;

/// Result alias for transport obfuscation operations.
pub type Result<T> = core::result::Result<T, Error>;

/// An obfuscation or proxying error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An underlying I/O error.
    Io(std::io::Error),
    /// A secret was provided but was shorter than the required 16 bytes.
    InvalidSecretLength(usize),
    /// A random obfuscation init could not be produced after repeated attempts.
    InvalidInit,
    /// A TLS record carried an unknown protocol version.
    UnknownProtocolVersion([u8; 2]),
    /// A TLS record had an unexpected type.
    UnexpectedRecordType {
        /// Description of the expected record type.
        expected: &'static str,
        /// The record type that was read.
        actual: u8,
    },
    /// The first handshake record was too short to carry the random.
    HandshakeRecordTooShort,
    /// No change-cipher-spec record was seen within the record limit.
    MissingChangeCipherSpec,
    /// The FakeTLS HMAC digest did not match.
    HmacMismatch,
    /// A generated ClientHello was too short to carry the random.
    ClientHelloTooShort(usize),
    /// A TLS record payload was too large for the 16-bit length field.
    RecordTooLarge(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::InvalidSecretLength(len) => {
                write!(f, "invalid secret size {len}, at least 16 bytes required")
            }
            Self::InvalidInit => f.write_str("failed to generate a valid obfuscation init"),
            Self::UnknownProtocolVersion(version) => {
                write!(f, "unknown protocol version {version:?}")
            }
            Self::UnexpectedRecordType { expected, actual } => {
                write!(
                    f,
                    "unexpected record type {actual:#04x}, expected {expected}"
                )
            }
            Self::HandshakeRecordTooShort => f.write_str("handshake record is too short"),
            Self::MissingChangeCipherSpec => f.write_str("missing change cipher spec record"),
            Self::HmacMismatch => f.write_str("hmac digest mismatch"),
            Self::ClientHelloTooShort(len) => {
                write!(f, "ClientHello is too short: {len} bytes")
            }
            Self::RecordTooLarge(len) => {
                write!(f, "TLS record payload too large: {len} bytes")
            }
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
