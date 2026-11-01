//! Small SHA-256/hex helpers shared by provenance verification and the
//! domain-separated digest writers.

use sha2::{Digest, Sha256};

/// Returns the lowercase hex SHA-256 of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    to_hex(&hasher.finalize())
}

/// Returns the raw SHA-256 of `bytes`.
pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// Encodes bytes as lowercase hex.
pub(crate) fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    out
}

/// A domain-separated SHA-256 writer matching `gen/layer_obligations.go`'s
/// `obligationHashWriter` and `gen/layer_execution_plan.go`'s
/// `layerExecutionHasher`.
pub(crate) struct DigestWriter {
    inner: Sha256,
}

impl DigestWriter {
    /// Starts a writer and hashes the length-prefixed domain.
    pub(crate) fn new(domain: &str) -> Self {
        let mut writer = DigestWriter {
            inner: Sha256::new(),
        };
        writer.string(domain);
        writer
    }

    /// Writes a length-prefixed UTF-8 string.
    pub(crate) fn string(&mut self, value: &str) {
        self.uint64(value.len() as u64);
        self.inner.update(value.as_bytes());
    }

    /// Writes length-prefixed bytes.
    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.uint64(value.len() as u64);
        self.inner.update(value);
    }

    /// Writes a little-endian 64-bit integer.
    pub(crate) fn uint64(&mut self, value: u64) {
        self.inner.update(value.to_le_bytes());
    }

    /// Writes a little-endian 32-bit integer.
    pub(crate) fn uint32(&mut self, value: u32) {
        self.inner.update(value.to_le_bytes());
    }

    /// Writes raw bytes without a length prefix.
    pub(crate) fn raw(&mut self, value: &[u8]) {
        self.inner.update(value);
    }

    /// Finalizes the digest.
    pub(crate) fn finish(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(&self.inner.finalize());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{sha256_hex, to_hex};

    #[test]
    fn hashes_empty_input() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn encodes_hex() {
        assert_eq!(to_hex(&[0x00, 0xab, 0xff]), "00abff");
    }
}
