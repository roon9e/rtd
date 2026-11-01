//! Versioned schema inputs: the layer manifest, the reviewed policy and the
//! staged `rtd.json` configuration.

use std::path::{Path, PathBuf};

use crate::json::{self, Json};

/// One immutable layer entry in the manifest.
#[derive(Debug, Clone)]
pub(crate) struct LayerEntry {
    /// Layer number.
    pub(crate) layer: i32,
    /// File name relative to the manifest directory.
    pub(crate) file: String,
    /// Upstream commit.
    pub(crate) commit: String,
    /// Upstream git blob (SHA-1 object id).
    pub(crate) blob: String,
    /// SHA-256 of the exact schema bytes.
    pub(crate) sha256: String,
}

/// One immutable overlay entry in the manifest.
#[derive(Debug, Clone)]
pub(crate) struct OverlayEntry {
    /// File name relative to the manifest directory.
    pub(crate) file: String,
    /// Upstream repository.
    pub(crate) repository: String,
    /// Upstream commit.
    pub(crate) commit: String,
    /// Upstream git blob.
    pub(crate) blob: String,
    /// Upstream path.
    pub(crate) path: String,
    /// SHA-256 of the exact schema bytes.
    pub(crate) sha256: String,
}

/// One promoted legacy method.
#[derive(Debug, Clone)]
pub(crate) struct LegacyMethod {
    /// The canonical method name that owns the promoted definition.
    pub(crate) name: String,
    /// The exact historical TL declaration.
    pub(crate) definition: String,
}

/// The parsed manifest.
#[derive(Debug, Clone)]
pub(crate) struct Manifest {
    /// The canonical layer.
    pub(crate) canonical_layer: i32,
    /// Upstream repository of the layer schemas.
    pub(crate) repository: String,
    /// Upstream source path.
    pub(crate) source_path: String,
    /// Overlays merged into every layer.
    pub(crate) overlays: Vec<OverlayEntry>,
    /// Promoted legacy methods.
    pub(crate) legacy_methods: Vec<LegacyMethod>,
    /// Layer entries ascending.
    pub(crate) layers: Vec<LayerEntry>,
}

/// The staged configuration in `schema/rtd.json`.
#[derive(Debug, Clone)]
pub(crate) struct RtdConfig {
    /// Layers to generate code for.
    pub(crate) enabled_layers: Vec<i32>,
    /// The canonical layer.
    pub(crate) canonical_layer: i32,
}

/// One reviewed policy resolution.
#[derive(Debug, Clone, Default)]
pub(crate) struct Resolution {
    /// Machine action.
    pub(crate) action: String,
    /// Generator hook name, when required.
    pub(crate) hook: String,
    /// Canonical semantic target, when required.
    pub(crate) target: String,
    /// Human note (never resolves an obligation by itself).
    pub(crate) note: String,
}

/// One policy entry keyed by a generated obligation digest.
#[derive(Debug, Clone)]
pub(crate) struct PolicyEntry {
    /// Domain-separated obligation key.
    pub(crate) key: String,
    /// Reviewed resolution.
    pub(crate) resolution: Resolution,
}

/// The reviewed policy.
#[derive(Debug, Clone)]
pub(crate) struct Policy {
    /// Entries in file order.
    pub(crate) entries: Vec<PolicyEntry>,
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))
}

/// Loads and validates the manifest.
///
/// # Errors
///
/// Returns a message when the file is missing, malformed or inconsistent.
pub(crate) fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let text = read(path)?;
    let json = json::parse(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let canonical_layer = json.field("canonical_layer")?.as_i64()? as i32;
    let repository = json.field("repository")?.as_str()?.to_owned();
    let source_path = json.field("source_path")?.as_str()?.to_owned();
    if canonical_layer <= 0 {
        return Err(format!(
            "manifest: invalid canonical layer {canonical_layer}"
        ));
    }
    let mut layers = Vec::new();
    for entry in json.field("layers")?.as_array()? {
        layers.push(LayerEntry {
            layer: entry.field("layer")?.as_i64()? as i32,
            file: entry.field("file")?.as_str()?.to_owned(),
            commit: entry.field("commit")?.as_str()?.to_owned(),
            blob: entry.field("blob")?.as_str()?.to_owned(),
            sha256: entry.field("sha256")?.as_str()?.to_owned(),
        });
    }
    if layers.is_empty() {
        return Err("manifest: no layers".to_owned());
    }
    let mut previous = 0;
    for entry in &layers {
        if entry.layer <= previous {
            return Err("manifest: layers must be strictly increasing".to_owned());
        }
        previous = entry.layer;
    }
    if !layers.iter().any(|entry| entry.layer == canonical_layer) {
        return Err(format!(
            "manifest: canonical layer {canonical_layer} is absent"
        ));
    }
    let mut overlays = Vec::new();
    if let Some(entries) = json.get("overlays") {
        for entry in entries.as_array()? {
            overlays.push(OverlayEntry {
                file: entry.field("file")?.as_str()?.to_owned(),
                repository: entry.field("repository")?.as_str()?.to_owned(),
                commit: entry.field("commit")?.as_str()?.to_owned(),
                blob: entry.field("blob")?.as_str()?.to_owned(),
                path: entry.field("path")?.as_str()?.to_owned(),
                sha256: entry.field("sha256")?.as_str()?.to_owned(),
            });
        }
    }
    let mut legacy_methods = Vec::new();
    if let Some(entries) = json.get("legacy_methods") {
        for entry in entries.as_array()? {
            legacy_methods.push(LegacyMethod {
                name: entry.field("name")?.as_str()?.to_owned(),
                definition: entry.field("definition")?.as_str()?.to_owned(),
            });
        }
    }
    Ok(Manifest {
        canonical_layer,
        repository,
        source_path,
        overlays,
        legacy_methods,
        layers,
    })
}

/// Loads the staged configuration.
///
/// # Errors
///
/// Returns a message when the file is missing or malformed.
pub(crate) fn load_rtd_config(path: &Path) -> Result<RtdConfig, String> {
    let text = read(path)?;
    let json = json::parse(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let enabled_layers = json
        .field("enabled_layers")?
        .as_array()?
        .iter()
        .map(Json::as_i64)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|value| value as i32)
        .collect();
    let canonical_layer = json.field("canonical_layer")?.as_i64()? as i32;
    Ok(RtdConfig {
        enabled_layers,
        canonical_layer,
    })
}

/// Loads the reviewed policy.
///
/// # Errors
///
/// Returns a message when the file is missing or malformed.
pub(crate) fn load_policy(path: &Path) -> Result<Policy, String> {
    let text = read(path)?;
    let json = json::parse(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut entries = Vec::new();
    for entry in json.field("entries")?.as_array()? {
        let key = entry.field("key")?.as_str()?.to_owned();
        let resolution = entry.field("resolution")?;
        entries.push(PolicyEntry {
            key,
            resolution: Resolution {
                action: resolution.field("action")?.as_str()?.to_owned(),
                hook: resolution
                    .get("hook")
                    .map(Json::as_str)
                    .transpose()?
                    .unwrap_or_default()
                    .to_owned(),
                target: resolution
                    .get("target")
                    .map(Json::as_str)
                    .transpose()?
                    .unwrap_or_default()
                    .to_owned(),
                note: resolution
                    .get("note")
                    .map(Json::as_str)
                    .transpose()?
                    .unwrap_or_default()
                    .to_owned(),
            },
        });
    }
    Ok(Policy { entries })
}

/// Verifies the SHA-256 of a schema file.
///
/// # Errors
///
/// Returns a message with the expected and actual digests on mismatch.
pub(crate) fn verify_sha256(path: &Path, expected: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let actual = crate::digest::sha256_hex(&bytes);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "{}: SHA-256 mismatch: expected {expected}, got {actual}",
            path.display()
        ));
    }
    Ok(bytes)
}

/// Resolves a manifest-relative path against the manifest directory.
pub(crate) fn resolve(base: &Path, file: &str) -> PathBuf {
    base.join(file.replace('/', std::path::MAIN_SEPARATOR_STR))
}

#[cfg(test)]
mod tests {
    use super::{load_manifest, load_policy, load_rtd_config};

    #[test]
    fn loads_real_manifest() {
        let manifest = load_manifest(std::path::Path::new("../../schema/layers/manifest.json"));
        let manifest = manifest.expect("manifest loads");
        assert_eq!(manifest.canonical_layer, 232);
        assert_eq!(manifest.layers.len(), 8);
    }

    #[test]
    fn loads_real_policy() {
        let policy = load_policy(std::path::Path::new("../../schema/layers/policy.json"));
        assert_eq!(policy.expect("policy loads").entries.len(), 249);
    }

    #[test]
    fn loads_rtd_config() {
        let config = load_rtd_config(std::path::Path::new("../../schema/rtd.json"));
        let config = config.expect("config loads");
        assert_eq!(
            config.enabled_layers,
            vec![225, 226, 227, 228, 229, 230, 231, 232]
        );
    }
}
