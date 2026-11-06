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
    /// The budget for the generated multi-layer sidecar.
    ///
    /// `td` mirrors 16 MiB / 400k lines for its nine-profile sidecar; rtd's
    /// representation is line-heavier, so the line ceiling is raised to fit the
    /// ninth exact profile. Bytes stay within 16 MiB.
    pub(crate) const fn layer_sidecar() -> Self {
        Budget {
            max_bytes: 16 * 1024 * 1024,
            max_lines: 500_000,
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
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{Budget, temp_path, verify, write_atomic};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A fresh, process-unique scratch directory that the caller owns.
    fn scratch() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "rtd-codegen-output-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("scratch directory");
        dir
    }

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

    #[test]
    fn write_atomic_commits_every_file_and_leaves_no_staging() {
        let dir = scratch();
        let first = dir.join("first.rs");
        let nested = dir.join("nested/deep/second.rs");
        let files = vec![
            (first.clone(), "first".to_owned()),
            (nested.clone(), "second".to_owned()),
        ];
        write_atomic(&files).expect("atomic write");
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "first");
        assert_eq!(std::fs::read_to_string(&nested).unwrap(), "second");
        assert!(!temp_path(&first).exists(), "staging file was not removed");
        assert!(!temp_path(&nested).exists(), "staging file was not removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_failure_leaves_prior_output_intact() {
        let dir = scratch();
        let kept = dir.join("kept.rs");
        std::fs::write(&kept, "old").expect("seed prior output");
        // The second target's parent is a regular file, so staging it fails
        // after the first file has been staged but before any commit.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "x").expect("seed blocker");
        let files = vec![
            (kept.clone(), "new".to_owned()),
            (blocker.join("nested.rs"), "boom".to_owned()),
        ];
        assert!(write_atomic(&files).is_err(), "staging must fail");
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "old",
            "prior output must survive a failed write"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_reports_fresh_stale_and_missing() {
        let dir = scratch();
        let fresh = dir.join("fresh.rs");
        let stale = dir.join("stale.rs");
        let missing = dir.join("missing.rs");
        let files = vec![
            (fresh.clone(), "same".to_owned()),
            (stale.clone(), "expected".to_owned()),
            (missing.clone(), "absent".to_owned()),
        ];
        std::fs::write(&fresh, "same").unwrap();
        std::fs::write(&stale, "changed").unwrap();
        let error = verify(&files).unwrap_err();
        assert!(error.contains("stale.rs differs"), "{error}");
        assert!(error.contains("missing.rs is missing"), "{error}");
        assert!(!error.contains("fresh.rs"), "{error}");

        std::fs::write(&stale, "expected").unwrap();
        std::fs::write(&missing, "absent").unwrap();
        verify(&files).expect("all files now match");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
