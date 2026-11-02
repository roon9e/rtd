//! Errors produced by the MTProto client runtime.

use core::fmt;

/// Result alias for the client runtime.
pub type Result<T> = core::result::Result<T, Error>;

/// A client runtime failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The transport failed to send or receive a frame.
    Transport(String),
    /// A socket could not be created or configured.
    Io(std::io::Error),
    /// The authorization-key exchange failed.
    Exchange(rtd_exchange::Error),
    /// MTProto encryption or decryption failed.
    Crypto(rtd_crypto::Error),
    /// A TL value could not be (de)serialized.
    Bin(rtd_bin::Error),
    /// A framed message could not be decoded.
    Decode(String),
    /// The peer closed the connection.
    Closed,
    /// The server returned an RPC error.
    Rpc(rtd_error::Error),
    /// The peer sent a message this runtime does not handle.
    Unexpected(String),
    /// A request was not answered within the bounded retry window.
    NoResult {
        /// The unacknowledged request's message id.
        message_id: i64,
    },
}

impl From<rtd_exchange::Error> for Error {
    fn from(error: rtd_exchange::Error) -> Self {
        Self::Exchange(error)
    }
}

impl From<rtd_crypto::Error> for Error {
    fn from(error: rtd_crypto::Error) -> Self {
        Self::Crypto(error)
    }
}

impl From<rtd_bin::Error> for Error {
    fn from(error: rtd_bin::Error) -> Self {
        Self::Bin(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(reason) => write!(f, "transport error: {reason}"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Exchange(error) => write!(f, "key exchange error: {error}"),
            Self::Crypto(error) => write!(f, "crypto error: {error}"),
            Self::Bin(error) => write!(f, "wire error: {error}"),
            Self::Decode(reason) => write!(f, "decode error: {reason}"),
            Self::Closed => f.write_str("connection closed"),
            Self::Rpc(error) => write!(f, "rpc error {}: {}", error.code(), error.message()),
            Self::Unexpected(reason) => write!(f, "unexpected message: {reason}"),
            Self::NoResult { message_id } => {
                write!(f, "request {message_id} was not answered")
            }
        }
    }
}

impl core::error::Error for Error {}
