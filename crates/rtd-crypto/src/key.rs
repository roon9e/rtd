//! Authorization key identity.
//!
//! The 2048-bit authorization key is never printed: [`Key`] and [`AuthKey`]
//! implement [`Debug`](core::fmt::Debug) and [`Display`](core::fmt::Display) so
//! that the secret bytes are replaced by a redaction marker, while the
//! non-secret 64-bit fingerprint remains available for routing and diagnostics.

use core::fmt;
use sha1::{Digest, Sha1};

/// A 2048-bit MTProto authorization key.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Key([u8; 256]);

impl Key {
    /// Length of the key in bytes.
    pub const LEN: usize = 256;

    /// Wraps an array of raw key bytes.
    #[must_use]
    pub const fn from_array(bytes: [u8; 256]) -> Self {
        Self(bytes)
    }

    /// Returns the raw key bytes.
    #[must_use]
    pub const fn as_array(&self) -> &[u8; 256] {
        &self.0
    }

    /// Returns the raw key bytes as a slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Reports whether the key is all zeroes.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 256]
    }

    /// Returns `auth_key_id`, the low 8 bytes of `SHA1(key)`.
    #[must_use]
    pub fn id(&self) -> [u8; 8] {
        let digest = Sha1::digest(self.0);
        let mut id = [0u8; 8];
        id.copy_from_slice(&digest[12..20]);
        id
    }

    /// Returns `aux_hash`, the first 8 bytes of `SHA1(key)`.
    #[must_use]
    pub fn aux_hash(&self) -> [u8; 8] {
        let digest = Sha1::digest(self.0);
        let mut hash = [0u8; 8];
        hash.copy_from_slice(&digest[0..8]);
        hash
    }

    /// Returns this key paired with its cached [`Key::id`].
    #[must_use]
    pub fn with_id(&self) -> AuthKey {
        AuthKey {
            value: *self,
            id: self.id(),
        }
    }
}

impl From<[u8; 256]> for Key {
    fn from(bytes: [u8; 256]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(<redacted>)")
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(redacted)")
    }
}

/// A [`Key`] together with its cached 64-bit fingerprint.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AuthKey {
    /// The secret key value.
    pub value: Key,
    /// The key fingerprint, equal to [`Key::id`].
    pub id: [u8; 8],
}

impl AuthKey {
    /// Creates an [`AuthKey`] from a [`Key`], deriving the fingerprint.
    #[must_use]
    pub fn new(value: Key) -> Self {
        Self {
            value,
            id: value.id(),
        }
    }

    /// Returns the key fingerprint as a little-endian `i64`.
    ///
    /// This is the value carried on the wire as `auth_key_id`.
    #[must_use]
    pub fn int_id(&self) -> i64 {
        i64::from_le_bytes(self.id)
    }

    /// Reports whether the key and its fingerprint are both all zeroes.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.value.is_zero() && self.id == [0u8; 8]
    }
}

impl fmt::Debug for AuthKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthKey { value: ")?;
        fmt::Display::fmt(&self.value, f)?;
        f.write_str(", id: ")?;
        for byte in self.id {
            write!(f, "{byte:02x}")?;
        }
        f.write_str(" }")
    }
}

impl fmt::Display for AuthKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(id: ")?;
        for byte in self.id {
            write!(f, "{byte:02x}")?;
        }
        f.write_str(")")
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthKey, Key};

    fn sequential_key() -> Key {
        let mut bytes = [0u8; 256];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = i as u8;
        }
        Key::from_array(bytes)
    }

    #[test]
    fn id_is_stable() {
        let key = sequential_key();
        assert_eq!(key.id(), key.id());
        assert_eq!(key.id().len(), 8);
        // SHA1(0..=255), low 8 bytes of the digest.
        assert_eq!(key.id(), [0x32, 0xd1, 0x58, 0x6e, 0xa4, 0x57, 0xdf, 0xc8]);
    }

    #[test]
    fn aux_hash_is_prefix_of_sha1() {
        let key = sequential_key();
        assert_eq!(key.aux_hash().len(), 8);
        assert_eq!(
            key.aux_hash(),
            [0x49, 0x16, 0xd6, 0xbd, 0xb7, 0xf7, 0x8e, 0x68]
        );
    }

    #[test]
    fn int_id_is_little_endian() {
        let key = AuthKey::new(sequential_key());
        let expected = [0x32, 0xd1, 0x58, 0x6e, 0xa4, 0x57, 0xdf, 0xc8];
        assert_eq!(key.int_id(), i64::from_le_bytes(expected));
    }

    #[test]
    fn formatting_redacts_key_bytes() {
        let key = sequential_key();
        let debug = format!("{key:?}");
        let display = format!("{key}");
        assert!(debug.contains("redacted"));
        assert!(!debug.contains("9c48"), "debug leaked key bytes");
        assert_eq!(display, "(redacted)");

        let auth = AuthKey::new(key);
        let auth_debug = format!("{auth:?}");
        let auth_display = format!("{auth}");
        assert!(!auth_debug.contains("9c48"), "debug leaked key bytes");
        // The fingerprint is not secret and is shown.
        assert!(auth_debug.contains("32d1586ea457dfc8"));
        assert_eq!(auth_display, "Key(id: 32d1586ea457dfc8)");
    }

    #[test]
    fn zero_detection() {
        // A truly zero key has both a zero value and a zero fingerprint.
        let zero = AuthKey {
            value: Key::from_array([0u8; 256]),
            id: [0u8; 8],
        };
        assert!(zero.is_zero());

        // A zero key value still has a non-zero fingerprint.
        let derived = AuthKey::new(Key::from_array([0u8; 256]));
        assert!(!derived.is_zero());
        assert!(!AuthKey::new(sequential_key()).is_zero());
    }
}
