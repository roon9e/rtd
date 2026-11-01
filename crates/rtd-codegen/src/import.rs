//! The schema import step: copy audited inputs from an upstream `td` (or
//! `tdesktop`) checkout into `schema/`, verifying every SHA-256 against the
//! manifest before writing anything.

use std::path::{Path, PathBuf};

use crate::output;
use crate::schema;

/// Imports schemas from `source` into `root`, preserving provenance.
///
/// # Errors
///
/// Returns a message on a read failure or a SHA-256 mismatch. Nothing is
/// written until every hash verifies.
pub(crate) fn run(root: &Path, source: &Path) -> Result<String, String> {
    let source_layers = source.join("layers");
    let manifest = schema::load_manifest(&source_layers.join("manifest.json"))?;
    let target_layers = root.join("schema/layers");

    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut verified = 0usize;

    for layer in &manifest.layers {
        let path = source_layers.join(&layer.file);
        let bytes = read_verified(&path, &layer.sha256)?;
        files.push((target_layers.join(&layer.file), bytes));
        verified += 1;
    }
    for overlay in &manifest.overlays {
        let path = source_layers.join(&overlay.file);
        let bytes = read_verified(&path, &overlay.sha256)?;
        let name = Path::new(&overlay.file)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("overlay {:?} has no file name", overlay.file))?;
        files.push((root.join("schema").join(name), bytes));
        verified += 1;
    }

    // The manifest and policy are copied verbatim; each schema was already
    // verified above.
    for name in ["manifest.json", "policy.json"] {
        let text = std::fs::read_to_string(source_layers.join(name))
            .map_err(|error| format!("read {name}: {error}"))?;
        files.push((target_layers.join(name), text));
    }
    let mt = std::fs::read_to_string(source.join("mt.tl"))
        .map_err(|error| format!("read mt.tl: {error}"))?;
    files.push((root.join("schema/mt.tl"), mt));

    output::write_atomic(&files)?;
    Ok(format!(
        "imported {} verified schema(s) and {} metadata file(s)",
        verified,
        files.len() - verified
    ))
}

fn read_verified(path: &Path, expected: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let actual = crate::digest::sha256_hex(&bytes);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "{}: SHA-256 mismatch: expected {expected}, got {actual}",
            path.display()
        ));
    }
    String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8", path.display()))
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn import_verifies_and_rejects_tampering() {
        let scratch = std::env::temp_dir().join("rtd-codegen-import-test");
        let _ = std::fs::remove_dir_all(&scratch);
        // A minimal source tree with one tampered layer.
        let source = scratch.join("source/layers");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(scratch.join("target")).unwrap();
        std::fs::write(
            source.join("manifest.json"),
            r#"{"canonical_layer":1,"repository":"r","source_path":"p","layers":[{"layer":1,"file":"layer-1.tl","commit":"0000000000000000000000000000000000000000","blob":"0000000000000000000000000000000000000000","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}]}"#,
        )
        .unwrap();
        std::fs::write(source.join("layer-1.tl"), "x#1 = X;\n").unwrap();
        std::fs::write(source.join("policy.json"), r#"{"version":1,"entries":[]}"#).unwrap();
        std::fs::write(source.join("mt.tl"), "// empty\n").unwrap();
        let error = run(&scratch.join("target"), &scratch.join("source")).unwrap_err();
        assert!(error.contains("SHA-256 mismatch"), "{error}");
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
