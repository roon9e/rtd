//! Formats generated Rust with `rustfmt` so committed output is stable under
//! `cargo fmt` and reproducible across runs.

use std::io::Write;
use std::process::{Command, Stdio};

/// Formats `source` with the workspace's rustfmt edition.
///
/// # Errors
///
/// Returns a message when `rustfmt` cannot be spawned, fails, or emits invalid
/// UTF-8.
pub(crate) fn rustfmt(source: &str) -> Result<String, String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn rustfmt: {error}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(source.as_bytes())
            .map_err(|error| format!("write to rustfmt: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for rustfmt: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "rustfmt failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| "rustfmt produced invalid UTF-8".to_owned())
}

#[cfg(test)]
mod tests {
    use super::rustfmt;

    #[test]
    fn formats_simple_source() {
        let formatted = rustfmt("fn  main( ){println!(\"x\");}").expect("rustfmt runs");
        assert!(formatted.contains("fn main()"));
    }
}
