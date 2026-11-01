//! Fail-closed conversion errors.

use core::fmt;

/// Result alias for the conversion hooks.
pub type Result<T> = core::result::Result<T, HookError>;

/// A conversion that cannot represent the value exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HookError {
    /// The value carries information the target shape cannot hold.
    Unrepresentable(&'static str),
    /// A required field was absent.
    Missing(&'static str),
    /// The value is not the variant the hook applies to.
    WrongVariant(&'static str),
}

impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unrepresentable(what) => write!(f, "cannot represent {what} in the target layer"),
            Self::Missing(what) => write!(f, "required field {what} is absent"),
            Self::WrongVariant(what) => write!(f, "unexpected variant for {what}"),
        }
    }
}

impl core::error::Error for HookError {}
