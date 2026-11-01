//! Telegram TL error types and parsing.
#![warn(missing_docs)]
//!
//! A Telegram RPC error is an integer code plus a message such as
//! `FLOOD_WAIT_3`. The message carries a *type* (`FLOOD_WAIT`) and, when the
//! last underscore-separated component is a number, a numeric *argument*
//! (`3`). This module reproduces that parsing exactly.

use core::fmt;
use core::time::Duration;

/// Error type of the `FLOOD_WAIT` error.
pub const ERR_FLOOD_WAIT: &str = "FLOOD_WAIT";
/// Error type of the `FLOOD_PREMIUM_WAIT` error.
pub const ERR_PREMIUM_FLOOD_WAIT: &str = "FLOOD_PREMIUM_WAIT";

/// Error types that are treated as flood waits.
pub const FLOOD_WAIT_ERRORS: [&str; 2] = [ERR_FLOOD_WAIT, ERR_PREMIUM_FLOOD_WAIT];

/// A Telegram RPC error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    code: i32,
    message: String,
    kind: String,
    argument: i64,
}

impl Error {
    /// Creates an error from a code and message, parsing its type and argument.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        let message = message.into();
        let (kind, argument) = extract_argument(&message);
        Self {
            code,
            message,
            kind,
            argument,
        }
    }

    /// Returns the integer error code.
    #[must_use]
    pub fn code(&self) -> i32 {
        self.code
    }

    /// Returns the original message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the parsed error type.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns the parsed numeric argument, or `0` when there is none.
    #[must_use]
    pub fn argument(&self) -> i64 {
        self.argument
    }

    /// Returns `true` if the error type equals `kind`.
    #[must_use]
    pub fn is_type(&self, kind: &str) -> bool {
        self.kind == kind
    }

    /// Returns `true` if the error code equals `code`.
    #[must_use]
    pub fn is_code(&self, code: i32) -> bool {
        self.code == code
    }

    /// Returns `true` if the error type is one of `kinds`.
    #[must_use]
    pub fn is_one_of<'a>(&self, kinds: impl IntoIterator<Item = &'a str>) -> bool {
        kinds.into_iter().any(|k| self.is_type(k))
    }

    /// Returns `true` if the error code is one of `codes`.
    #[must_use]
    pub fn is_code_one_of(&self, codes: impl IntoIterator<Item = i32>) -> bool {
        codes.into_iter().any(|c| self.is_code(c))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kind != self.message {
            write!(
                f,
                "rpc error code {}: {} ({})",
                self.code, self.kind, self.argument
            )
        } else {
            write!(f, "rpc error code {}: {}", self.code, self.message)
        }
    }
}

impl core::error::Error for Error {}

/// Splits a message into its type and numeric argument.
///
/// Mirrors the reference implementation: the message is split on `_`; every
/// component consisting solely of ASCII digits is taken as the argument (the
/// last one wins), and the type is the remaining components rejoined with `_`.
fn extract_argument(message: &str) -> (String, i64) {
    if message.is_empty() {
        return (String::new(), 0);
    }

    let parts: Vec<&str> = message.split('_').collect();
    if parts.len() < 2 {
        return (message.to_owned(), 0);
    }

    let mut non_digit: Vec<&str> = Vec::new();
    let mut argument: i64 = 0;
    for part in &parts {
        if part.bytes().all(|b| b.is_ascii_digit()) {
            match part.parse::<i64>() {
                Ok(value) => argument = value,
                // Should be unreachable for a digits-only component.
                Err(_) => return (message.to_owned(), argument),
            }
        } else {
            non_digit.push(part);
        }
    }

    (non_digit.join("_"), argument)
}

/// Returns the wait duration if `error` is a flood-wait error, and `None`
/// otherwise.
#[must_use]
pub fn as_flood_wait(error: &Error) -> Option<Duration> {
    if error.is_one_of(FLOOD_WAIT_ERRORS) {
        let secs = u64::try_from(error.argument()).unwrap_or(0);
        return Some(Duration::from_secs(secs));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_type_and_argument() {
        let e = Error::new(420, "FLOOD_WAIT_3");
        assert_eq!(e.code(), 420);
        assert_eq!(e.message(), "FLOOD_WAIT_3");
        assert_eq!(e.kind(), "FLOOD_WAIT");
        assert_eq!(e.argument(), 3);
    }

    #[test]
    fn type_only_message() {
        let e = Error::new(400, "AUTH_KEY_INVALID");
        assert_eq!(e.kind(), "AUTH_KEY_INVALID");
        assert_eq!(e.argument(), 0);
    }

    #[test]
    fn multi_component_type() {
        let e = Error::new(303, "FILE_MIGRATE_2");
        assert_eq!(e.kind(), "FILE_MIGRATE");
        assert_eq!(e.argument(), 2);
    }

    #[test]
    fn empty_message() {
        let e = Error::new(0, "");
        assert_eq!(e.kind(), "");
        assert_eq!(e.argument(), 0);
    }

    #[test]
    fn display_matches_reference() {
        assert_eq!(
            Error::new(420, "FLOOD_WAIT_3").to_string(),
            "rpc error code 420: FLOOD_WAIT (3)"
        );
        assert_eq!(
            Error::new(400, "AUTH_KEY_INVALID").to_string(),
            "rpc error code 400: AUTH_KEY_INVALID"
        );
    }

    #[test]
    fn flood_wait_duration() {
        let d = as_flood_wait(&Error::new(420, "FLOOD_WAIT_7")).unwrap();
        assert_eq!(d, Duration::from_secs(7));
        assert_eq!(as_flood_wait(&Error::new(400, "AUTH_KEY_INVALID")), None);
    }

    #[test]
    fn predicates() {
        let e = Error::new(303, "FILE_MIGRATE_2");
        assert!(e.is_type("FILE_MIGRATE"));
        assert!(e.is_code(303));
        assert!(e.is_one_of(["PHONE_MIGRATE", "FILE_MIGRATE"]));
        assert!(e.is_code_one_of([303, 400]));
    }
}
