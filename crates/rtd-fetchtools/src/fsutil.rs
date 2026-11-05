//! Small filesystem and hashing helpers shared by the fetchers.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::Result;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns the lowercase hex SHA-256 of `data`.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex::encode(digest)
}

/// Writes `data` to `path` atomically via a sibling temp file and a rename.
///
/// Mirrors Go's `writeFileAtomic`: creates the parent directory, writes a temp
/// file with mode `0644` (Unix), then renames it into place, replacing any
/// existing file.
///
/// # Errors
///
/// Returns an error when the directory cannot be created or the file cannot be
/// written or renamed.
pub fn write_file_atomic(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let temp = temp_path(path);
    std::fs::write(&temp, data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o644))?;
    }
    match std::fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            // Windows cannot rename over an existing file; replace then retry.
            if path.exists() {
                let _ = std::fs::remove_file(path);
                if std::fs::rename(&temp, path).is_ok() {
                    return Ok(());
                }
            }
            let _ = std::fs::remove_file(&temp);
            Err(rename_error.into())
        }
    }
}

/// Builds a sibling temp path for the atomic write.
fn temp_path(path: &Path) -> std::path::PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp_name = format!(".{name}.{}.{counter}.tmp", std::process::id());
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(temp_name),
        _ => std::path::PathBuf::from(temp_name),
    }
}
