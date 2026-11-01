//! Errors produced by MTProto framing and codecs.

use core::fmt;

/// Result alias for MTProto framing operations.
pub type Result<T> = core::result::Result<T, Error>;

/// An MTProto framing or codec error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A borrowed TL decoding error from [`rtd_bin`].
    Bin(rtd_bin::Error),
    /// A declared length was negative or above the accepted bound.
    InvalidLength {
        /// The offending length.
        length: i64,
        /// Short description of what was being decoded.
        context: &'static str,
    },
    /// A packed payload expanded beyond the configured limit.
    DecompressionBomb {
        /// Maximum number of expanded bytes accepted.
        limit: usize,
        /// Number of bytes read before the limit was hit.
        decompressed: usize,
    },
    /// An unencrypted message carried a non-zero auth-key id.
    UnexpectedAuthKeyId(i64),
    /// An I/O error, for example while (de)compressing a packed payload.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bin(err) => write!(f, "{err}"),
            Self::InvalidLength { length, context } => {
                write!(f, "invalid length {length} while decoding {context}")
            }
            Self::DecompressionBomb {
                limit,
                decompressed,
            } => write!(
                f,
                "decompression bomb: expanded {decompressed} bytes past the {limit} byte limit"
            ),
            Self::UnexpectedAuthKeyId(id) => {
                write!(f, "unexpected auth_key_id {id} of plaintext message")
            }
            Self::Io(err) => write!(f, "io error: {err}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Bin(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<rtd_bin::Error> for Error {
    fn from(err: rtd_bin::Error) -> Self {
        Self::Bin(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
