//! Errors produced by TL (de)serialization.

use core::fmt;

/// Result alias for TL (de)serialization.
pub type Result<T> = core::result::Result<T, Error>;

/// A TL (de)serialization error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The buffer ended before the requested number of bytes were available.
    UnexpectedEof {
        /// Number of bytes the operation needed.
        needed: usize,
        /// Number of bytes that were actually available.
        remaining: usize,
    },
    /// A constructor identifier did not match the expected value.
    UnexpectedId {
        /// Identifier that was read.
        id: u32,
        /// Identifier the decoder expected.
        expected: u32,
    },
    /// A length (string, bytes or vector) was rejected.
    InvalidLength {
        /// The offending length.
        length: usize,
        /// Short description of what was being decoded.
        context: &'static str,
    },
    /// A vector declared a negative element count.
    NegativeVectorCount(i32),
    /// A `string` value was not valid UTF-8.
    InvalidUtf8,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { needed, remaining } => write!(
                f,
                "unexpected end of buffer: needed {needed} bytes, {remaining} remaining"
            ),
            Self::UnexpectedId { id, expected } => write!(
                f,
                "unexpected constructor id {id:#010x}, expected {expected:#010x}"
            ),
            Self::InvalidLength { length, context } => {
                write!(f, "invalid length {length} while decoding {context}")
            }
            Self::NegativeVectorCount(count) => {
                write!(f, "negative vector element count {count}")
            }
            Self::InvalidUtf8 => write!(f, "string value is not valid UTF-8"),
        }
    }
}

impl core::error::Error for Error {}
