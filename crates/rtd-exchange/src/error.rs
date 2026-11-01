//! Errors produced by the authorization-key exchange.

use core::fmt;

/// Result alias for the exchange flows.
pub type Result<T> = core::result::Result<T, Error>;

/// An authorization-key exchange failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The underlying stream failed to send or receive.
    Stream(String),
    /// A plaintext message could not be decoded.
    Decode(String),
    /// The peer used an already-established key instead of performing exchange.
    ///
    /// The caller should resolve the key and handle the frame as an encrypted
    /// message rather than replying that the key is unknown.
    UnexpectedEncrypted {
        /// The auth-key id the encrypted frame was sent with.
        auth_key_id: i64,
    },
    /// A nonce did not match the value this side generated.
    NonceMismatch,
    /// No candidate public key matched the fingerprint the server offered.
    KeyFingerprintNotFound,
    /// The client requested a different datacenter than this server serves.
    WrongDc {
        /// The datacenter this server serves.
        want: i32,
        /// The datacenter the client requested.
        got: i32,
    },
    /// The server refused the exchange (`server_DH_params_fail`).
    ServerRefused,
    /// The server's `pq` was larger than the accepted bound.
    BadPq,
    /// The derived authorization key did not fit the key size.
    KeyTooLarge,
    /// The server's verification hash did not match.
    HashMismatch,
    /// A cryptographic operation failed.
    Crypto(rtd_crypto::Error),
    /// A TL value could not be (de)serialized.
    Bin(rtd_bin::Error),
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stream(reason) => write!(f, "stream error: {reason}"),
            Self::Decode(reason) => write!(f, "decode error: {reason}"),
            Self::UnexpectedEncrypted { auth_key_id } => write!(
                f,
                "unexpected encrypted message during key exchange (auth key id {auth_key_id:#018x})"
            ),
            Self::NonceMismatch => write!(f, "nonce mismatch"),
            Self::KeyFingerprintNotFound => write!(f, "no matching public key fingerprint"),
            Self::WrongDc { want, got } => write!(f, "wrong datacenter: want {want}, got {got}"),
            Self::ServerRefused => write!(f, "server refused the key exchange"),
            Self::BadPq => write!(f, "server provided an out-of-range pq"),
            Self::KeyTooLarge => write!(f, "derived authorization key is too large"),
            Self::HashMismatch => write!(f, "key exchange verification hash mismatch"),
            Self::Crypto(error) => write!(f, "crypto error: {error}"),
            Self::Bin(error) => write!(f, "wire error: {error}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Crypto(error) => Some(error),
            Self::Bin(error) => Some(error),
            _ => None,
        }
    }
}
