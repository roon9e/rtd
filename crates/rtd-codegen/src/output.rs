//! Deterministic, atomic output writing and the source budget.

use std::path::Path;

/// The declared budget for the generated multi-layer sidecar.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Budget {
    /// Maximum bytes.
    pub(crate) max_bytes: usize,
    /// Maximum lines.
    pub(crate) max_lines: usize,
}

impl Budget {
    /// The budget mirrored from `td`: 16 MiB / 400k lines.
    pub(crate) const fn layer_sidecar() -> Self {
        Budget {
            max_bytes: 16 * 1024 * 1024,
            max_lines: 400_000,
        }
    }

    /// Measures `sources` and fails when the budget is exceeded.
    ///
    /// # Errors
    ///
    /// Returns a message reporting the measured size and the budget.
    pub(crate) fn check(&self, sources: &[String]) -> Result<(), String> {
        let bytes: usize = sources.iter().map(String::len).sum();
        let lines: usize = sources.iter().map(|source| source.lines().count()).sum();
        if bytes > self.max_bytes || lines > self.max_lines {
            return Err(format!(
                "source budget exceeded: {bytes} bytes / {lines} lines exceeds {} bytes / {} lines",
                self.max_bytes, self.max_lines
            ));
        }
        Ok(())
    }
}

fn temp_path(target: &Path) -> std::path::PathBuf {
    let mut name = target
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".rtd-codegen.tmp");
    target.with_file_name(name)
}

/// Writes `files` atomically: every staged file is written first, then renamed.
///
/// # Errors
///
/// Returns a message when a directory cannot be created, a file cannot be
/// written, or a staged file cannot be committed.
pub(crate) fn write_atomic(files: &[(std::path::PathBuf, String)]) -> Result<(), String> {
    let mut staged = Vec::with_capacity(files.len());
    for (target, content) in files {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        let temp = temp_path(target);
        std::fs::write(&temp, content)
            .map_err(|error| format!("write {}: {error}", temp.display()))?;
        staged.push((temp, target.clone()));
    }
    for (temp, target) in &staged {
        std::fs::rename(temp, target)
            .map_err(|error| format!("commit {}: {error}", target.display()))?;
    }
    Ok(())
}

/// Compares `files` against their committed content.
///
/// # Errors
///
/// Returns a message naming every file that differs or is missing.
pub(crate) fn verify(files: &[(std::path::PathBuf, String)]) -> Result<(), String> {
    let mut mismatches = Vec::new();
    for (target, expected) in files {
        match std::fs::read_to_string(target) {
            Ok(actual) if &actual == expected => {}
            Ok(_) => mismatches.push(format!("{} differs", target.display())),
            Err(_) => mismatches.push(format!("{} is missing", target.display())),
        }
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "generated sources are stale:\n{}",
            mismatches.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Budget;

    #[test]
    fn budget_rejects_oversized() {
        let budget = Budget {
            max_bytes: 4,
            max_lines: 100,
        };
        assert!(budget.check(&["hello".to_owned()]).is_err());
        assert!(budget.check(&["hi".to_owned()]).is_ok());
    }

    #[test]
    fn budget_rejects_too_many_lines() {
        let budget = Budget {
            max_bytes: 1024,
            max_lines: 1,
        };
        assert!(budget.check(&["a\nb".to_owned()]).is_err());
    }
}
