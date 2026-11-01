//! Error type shared by the cryptographic primitives.

use core::fmt;

/// Result alias for cryptographic operations.
pub type Result<T> = core::result::Result<T, Error>;

/// An error produced by a cryptographic primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A payload passed to [`crate::rsa::rsa_pad`] exceeded the 144-byte limit.
    RsaDataTooLong {
        /// The offending payload length.
        len: usize,
    },
    /// The RSA-encrypted block could not be unwrapped.
    InvalidEncryptedData,
    /// A decrypted block did not match its embedded hash.
    HashMismatch,
    /// A Diffie-Hellman or SRP parameter failed validation.
    DhParams {
        /// Human-readable reason for the rejection.
        reason: &'static str,
    },
    /// A big integer did not fit in the required fixed width.
    IntegerTooLarge,
    /// An operation required a non-zero authorization key.
    ZeroAuthKey,
    /// An SRP input or computation failed.
    Srp {
        /// Human-readable reason for the failure.
        reason: &'static str,
    },
    /// A PEM or DER encoded key could not be parsed.
    KeyParse(String),
    /// A buffer length was not a multiple of the AES block size.
    InvalidBlockLength {
        /// The offending length.
        len: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RsaDataTooLong { len } => {
                write!(f, "data length {len} is bigger than 144")
            }
            Self::InvalidEncryptedData => write!(f, "invalid encrypted data"),
            Self::HashMismatch => write!(f, "hash mismatch"),
            Self::DhParams { reason } => write!(f, "invalid DH parameters: {reason}"),
            Self::IntegerTooLarge => write!(f, "integer does not fit the required width"),
            Self::ZeroAuthKey => write!(f, "authorization key is zero"),
            Self::Srp { reason } => write!(f, "SRP failure: {reason}"),
            Self::KeyParse(message) => write!(f, "failed to parse key: {message}"),
            Self::InvalidBlockLength { len } => {
                write!(
                    f,
                    "data length {len} is not a multiple of the AES block size"
                )
            }
        }
    }
}

impl core::error::Error for Error {}
