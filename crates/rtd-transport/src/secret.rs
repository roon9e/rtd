//! MTProxy secret parsing.
//!
//! An MTProxy secret is a hexadecimal string that selects the obfuscation layer
//! in use and carries the key both endpoints derive their streams from:
//!
//! - no prefix: a plain obfuscated2 secret (exactly 16 bytes);
//! - a `dd` prefix: a secured obfuscated2 secret;
//! - an `ee` prefix: a FakeTLS secret, optionally followed by the TLS cloak
//!   host.
//!
//! The parsed [`Secret`] composes with [`crate::obfuscated2`] (plain and
//! secured secrets) and [`crate::faketls`] (FakeTLS secrets) through
//! [`Secret::obfuscated2_key`] and [`Secret::faketls_key`], which reject a
//! secret whose mode does not match the transport.
//!
//! See <https://core.telegram.org/mtproto/mtproto-transports#transport-obfuscation>.

use crate::error::{Error, Result};

/// The obfuscation mode selected by an MTProxy secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretMode {
    /// A plain obfuscated2 secret with no mode prefix.
    Plain,
    /// A secured (`dd`) obfuscated2 secret.
    Secured,
    /// A FakeTLS (`ee`) secret.
    FakeTls,
}

impl SecretMode {
    /// Returns a human-readable name for the mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Secured => "secured",
            Self::FakeTls => "faketls",
        }
    }
}

/// A parsed MTProxy secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secret {
    mode: SecretMode,
    key: Vec<u8>,
    cloak_host: String,
}

impl Secret {
    /// The number of key bytes required by the obfuscation layers.
    pub const KEY_LENGTH: usize = 16;

    /// Parses a human MTProxy secret string.
    ///
    /// A plain secret is exactly [`Secret::KEY_LENGTH`] bytes. A longer secret
    /// must start with the `dd` (secured) or `ee` (FakeTLS) mode prefix; for a
    /// FakeTLS secret the bytes after the 16-byte key are the TLS cloak host.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidSecretHex`] if `secret` is not an even-length
    /// hexadecimal string, [`Error::InvalidSecretLength`] if it decodes to
    /// fewer than [`Secret::KEY_LENGTH`] bytes, and
    /// [`Error::InvalidSecretPrefix`] if a longer secret does not begin with
    /// `dd` or `ee`.
    pub fn parse(secret: &str) -> Result<Self> {
        let bytes = decode_hex(secret)?;
        if bytes.len() < Self::KEY_LENGTH {
            return Err(Error::InvalidSecretLength(bytes.len()));
        }
        if bytes.len() == Self::KEY_LENGTH {
            return Ok(Self {
                mode: SecretMode::Plain,
                key: bytes,
                cloak_host: String::new(),
            });
        }
        match bytes[0] {
            0xdd => Ok(Self {
                mode: SecretMode::Secured,
                key: bytes[1..].to_vec(),
                cloak_host: String::new(),
            }),
            0xee => Ok(Self {
                mode: SecretMode::FakeTls,
                key: bytes[1..=Self::KEY_LENGTH].to_vec(),
                cloak_host: String::from_utf8_lossy(&bytes[Self::KEY_LENGTH + 1..]).into_owned(),
            }),
            other => Err(Error::InvalidSecretPrefix(other)),
        }
    }

    /// Returns the obfuscation mode selected by the secret.
    #[must_use]
    pub const fn mode(&self) -> SecretMode {
        self.mode
    }

    /// Returns the raw key bytes carried by the secret.
    #[must_use]
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    /// Returns the FakeTLS cloak host, or an empty string for other modes.
    #[must_use]
    pub fn cloak_host(&self) -> &str {
        &self.cloak_host
    }

    /// Returns the key to use for an obfuscated2 connection.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SecretModeMismatch`] when the secret is a FakeTLS
    /// (`ee`) secret.
    pub fn obfuscated2_key(&self) -> Result<&[u8]> {
        match self.mode {
            SecretMode::Plain | SecretMode::Secured => Ok(&self.key),
            SecretMode::FakeTls => Err(Error::SecretModeMismatch {
                expected: "obfuscated2",
                actual: self.mode,
            }),
        }
    }

    /// Returns the key to use for a FakeTLS connection.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SecretModeMismatch`] when the secret is plain or
    /// secured (`dd`).
    pub fn faketls_key(&self) -> Result<&[u8]> {
        match self.mode {
            SecretMode::FakeTls => Ok(&self.key),
            SecretMode::Plain | SecretMode::Secured => Err(Error::SecretModeMismatch {
                expected: "faketls",
                actual: self.mode,
            }),
        }
    }
}

impl core::str::FromStr for Secret {
    type Err = Error;

    fn from_str(secret: &str) -> Result<Self> {
        Self::parse(secret)
    }
}

/// Decodes an even-length hexadecimal string into bytes.
fn decode_hex(input: &str) -> Result<Vec<u8>> {
    let (pairs, remainder) = input.as_bytes().as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(Error::InvalidSecretHex);
    }
    let mut out = Vec::with_capacity(pairs.len());
    for pair in pairs {
        let hi = hex_digit(pair[0]).ok_or(Error::InvalidSecretHex)?;
        let lo = hex_digit(pair[1]).ok_or(Error::InvalidSecretHex)?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

/// Returns the numeric value of one hexadecimal digit, or `None`.
const fn hex_digit(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Secret, SecretMode};
    use crate::error::Error;

    fn hex(value: &str) -> Vec<u8> {
        (0..value.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn parses_plain_secret() {
        let secret = Secret::parse("52a493bdfb90eea55739eabff2d92a14").unwrap();
        assert_eq!(secret.mode(), SecretMode::Plain);
        assert_eq!(secret.key(), hex("52a493bdfb90eea55739eabff2d92a14"));
        assert!(secret.cloak_host().is_empty());
        assert_eq!(
            secret.obfuscated2_key().unwrap(),
            hex("52a493bdfb90eea55739eabff2d92a14")
        );
    }

    #[test]
    fn parses_secured_secret() {
        let secret = Secret::parse("ddf05fb7acb549be047a7c585116581418").unwrap();
        assert_eq!(secret.mode(), SecretMode::Secured);
        assert_eq!(secret.key(), hex("f05fb7acb549be047a7c585116581418"));
        assert_eq!(
            secret.obfuscated2_key().unwrap(),
            hex("f05fb7acb549be047a7c585116581418")
        );
    }

    #[test]
    fn parses_faketls_secret_with_cloak_host() {
        let secret =
            Secret::parse("ee852380f362a09343efb4690c4e17862e676f6f676c652e636f6d").unwrap();
        assert_eq!(secret.mode(), SecretMode::FakeTls);
        assert_eq!(secret.key(), hex("852380f362a09343efb4690c4e17862e"));
        assert_eq!(secret.cloak_host(), "google.com");
        assert_eq!(
            secret.faketls_key().unwrap(),
            hex("852380f362a09343efb4690c4e17862e")
        );
    }

    #[test]
    fn parses_faketls_secret_without_cloak_host() {
        let secret = Secret::parse("ee852380f362a09343efb4690c4e17862e").unwrap();
        assert_eq!(secret.mode(), SecretMode::FakeTls);
        assert_eq!(secret.key(), hex("852380f362a09343efb4690c4e17862e"));
        assert!(secret.cloak_host().is_empty());
    }

    #[test]
    fn rejects_too_short_secret() {
        assert!(matches!(
            Secret::parse("52a493bdfb90eea55739eabff2d92a"),
            Err(Error::InvalidSecretLength(15))
        ));
        assert!(matches!(
            Secret::parse(""),
            Err(Error::InvalidSecretLength(0))
        ));
    }

    #[test]
    fn rejects_non_hex_secret() {
        assert!(matches!(
            Secret::parse("zz493bdfb90eea55739eabff2d92a14"),
            Err(Error::InvalidSecretHex)
        ));
        assert!(matches!(Secret::parse("abc"), Err(Error::InvalidSecretHex)));
    }

    #[test]
    fn rejects_unknown_prefix() {
        assert!(matches!(
            Secret::parse("52a493bdfb90eea55739eabff2d92a1422"),
            Err(Error::InvalidSecretPrefix(0x52))
        ));
    }

    #[test]
    fn wrong_mode_is_rejected() {
        let secured = Secret::parse("ddf05fb7acb549be047a7c585116581418").unwrap();
        assert!(matches!(
            secured.faketls_key(),
            Err(Error::SecretModeMismatch {
                expected: "faketls",
                actual: SecretMode::Secured,
            })
        ));

        let plain = Secret::parse("52a493bdfb90eea55739eabff2d92a14").unwrap();
        assert!(matches!(
            plain.faketls_key(),
            Err(Error::SecretModeMismatch {
                expected: "faketls",
                actual: SecretMode::Plain,
            })
        ));

        let faketls =
            Secret::parse("ee852380f362a09343efb4690c4e17862e676f6f676c652e636f6d").unwrap();
        assert!(matches!(
            faketls.obfuscated2_key(),
            Err(Error::SecretModeMismatch {
                expected: "obfuscated2",
                actual: SecretMode::FakeTls,
            })
        ));
    }

    #[test]
    fn from_str_matches_parse() {
        use core::str::FromStr;
        let parsed = Secret::parse("ddf05fb7acb549be047a7c585116581418").unwrap();
        let via_trait = Secret::from_str("ddf05fb7acb549be047a7c585116581418").unwrap();
        assert_eq!(parsed, via_trait);
    }

    #[test]
    fn secured_secret_composes_with_obfuscated2() {
        use crate::obfuscated2::Obfuscated2;
        use std::io::{Cursor, Read, Write};

        let secret = Secret::parse("ddf05fb7acb549be047a7c585116581418").unwrap();
        let key = secret.obfuscated2_key().unwrap();

        let mut client =
            Obfuscated2::handshake(Cursor::new(Vec::new()), [0xdd; 4], 2, key).unwrap();
        client.write_all(b"hello").unwrap();
        let wire = client.into_inner().into_inner();

        let (mut server, metadata) = Obfuscated2::accept(Cursor::new(wire), key).unwrap();
        assert_eq!(metadata.dc, 2);
        let mut received = [0u8; 5];
        server.read_exact(&mut received).unwrap();
        assert_eq!(&received, b"hello");
    }

    #[test]
    fn faketls_secret_composes_with_faketls() {
        use crate::faketls::{verify_client_hello, write_client_hello};

        let secret =
            Secret::parse("ee852380f362a09343efb4690c4e17862e676f6f676c652e636f6d").unwrap();
        let key = secret.faketls_key().unwrap();

        let mut record = Vec::new();
        let random = write_client_hello(&mut record, 42, secret.cloak_host(), key).unwrap();
        assert_eq!(verify_client_hello(&record, key).unwrap(), random);
        // A key from an unrelated secret must not verify.
        assert!(verify_client_hello(&record, b"wrongsecretvalue!").is_err());
    }
}
