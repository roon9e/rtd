//! Build information and instance identity for the edge.

use rtd_util::random::random_bytes;

/// Compiled-in build information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildInfo {
    /// The crate version.
    pub version: &'static str,
    /// The build commit, or `"unknown"` when not provided at compile time.
    pub commit: &'static str,
}

/// Returns the compiled-in build information.
#[must_use]
pub fn build_info() -> BuildInfo {
    BuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        commit: option_env!("RTD_GIT_SHA").unwrap_or("unknown"),
    }
}

/// Resolves the instance identity: the configured value, or a random one.
///
/// A configured identity is stable across restarts; a generated one is logged at
/// startup so log lines remain attributable.
#[must_use]
pub fn resolve_instance_id(configured: Option<&str>) -> String {
    match configured {
        Some(id) => id.to_owned(),
        None => format!("edge-{}", hex(&random_bytes(8))),
    }
}

/// Encodes bytes as lowercase hexadecimal.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{build_info, resolve_instance_id};

    #[test]
    fn configured_identity_is_kept() {
        assert_eq!(resolve_instance_id(Some("edge-a")), "edge-a");
    }

    #[test]
    fn generated_identity_is_prefixed() {
        let id = resolve_instance_id(None);
        assert!(id.starts_with("edge-"));
        assert_ne!(id, resolve_instance_id(None));
    }

    #[test]
    fn build_info_has_a_version() {
        assert!(!build_info().version.is_empty());
    }
}
