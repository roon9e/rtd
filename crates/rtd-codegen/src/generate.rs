//! Orchestrates schema loading, analysis and emission into the committed crates.

use std::path::{Path, PathBuf};

use crate::load;
use crate::obligations;
use crate::output::{self, Budget};
use crate::routes;
use crate::schema;

/// The workspace-relative layout of the generated crates.
pub(crate) struct Layout {
    /// `crates/rtd-tg/src/lib.rs`.
    pub(crate) tg: PathBuf,
    /// `crates/rtd-tlprofile/src/lib.rs`.
    pub(crate) tlprofile: PathBuf,
    /// `crates/rtd-mt/src/lib.rs`.
    pub(crate) mt: PathBuf,
}

impl Layout {
    /// Resolves the layout relative to a workspace root.
    pub(crate) fn new(root: &Path) -> Self {
        Layout {
            tg: root.join("crates/rtd-tg/src/lib.rs"),
            tlprofile: root.join("crates/rtd-tlprofile/src/lib.rs"),
            mt: root.join("crates/rtd-mt/src/lib.rs"),
        }
    }
}

/// The generated sources, keyed by target path.
pub(crate) struct Generated {
    /// Files to write.
    pub(crate) files: Vec<(PathBuf, String)>,
    /// Diagnostic summary.
    pub(crate) summary: String,
}

/// Runs the full pipeline and returns the generated sources.
///
/// # Errors
///
/// Returns a message on any load, analysis, classification or emission error,
/// including unresolved obligations, stale policy and budget breaches.
pub(crate) fn generate(root: &Path) -> Result<Generated, String> {
    let manifest_path = root.join("schema/layers/manifest.json");
    let policy_path = root.join("schema/layers/policy.json");
    let config_path = root.join("schema/rtd.json");

    let (universe, _manifest) = load::load_universe(&manifest_path)?;
    let policy = schema::load_policy(&policy_path)?;
    let config = schema::load_rtd_config(&config_path)?;
    if config.canonical_layer != universe.canonical_layer {
        return Err(format!(
            "rtd.json canonical layer {} disagrees with manifest {}",
            config.canonical_layer, universe.canonical_layer
        ));
    }
    let plan = obligations::analyze(&universe, &policy)?;
    let unresolved = plan.unresolved();
    if !unresolved.is_empty() {
        let mut message = format!(
            "E_UNREVIEWED_LAYER_DIFFERENCE: {} unreviewed difference(s):",
            unresolved.len()
        );
        for obligation in unresolved.iter().take(50) {
            message.push('\n');
            message.push_str("  ");
            message.push_str(&obligation.summary());
        }
        return Err(message);
    }
    let enabled = config.enabled_layers.clone();
    let table = routes::build_route_table(&universe, &plan, &enabled)?;
    let plans = routes::build_plans(&universe, &plan, &enabled)?;

    let canonical = universe.canonical();
    let tg = crate::format::rustfmt(&crate::emit_tg::emit(canonical).map_err(|error| error.0)?)?;
    let tlprofile = crate::format::rustfmt(&crate::emit_profile::emit(
        &universe, &plan, &table, &plans, &config,
    )?)?;
    let mt = crate::format::rustfmt(&crate::emit_mt::emit(root)?)?;

    Budget::layer_sidecar().check(std::slice::from_ref(&tlprofile))?;

    let layout = Layout::new(root);
    let summary = format!(
        "canonical={} enabled={:?} routes={} body_plans={} preflight_plans={} result_plans={} profile_bytes={} profile_lines={}",
        universe.canonical_layer,
        enabled,
        table.routes.len(),
        plans.body_count(),
        plans.preflight_count(),
        plans.result_count(),
        tlprofile.len(),
        tlprofile.lines().count(),
    );
    let summary = format!("{summary} {}", table.mode_summary());
    Ok(Generated {
        files: vec![
            (layout.tg, tg),
            (layout.tlprofile, tlprofile),
            (layout.mt, mt),
        ],
        summary,
    })
}

/// Writes generated sources to disk.
///
/// # Errors
///
/// Returns a message on generation or write failure.
pub(crate) fn run_generate(root: &Path) -> Result<String, String> {
    let generated = generate(root)?;
    output::write_atomic(&generated.files)?;
    Ok(generated.summary)
}

/// Verifies committed sources match a fresh generation.
///
/// # Errors
///
/// Returns a message on generation failure or when a file differs.
pub(crate) fn run_verify(root: &Path) -> Result<String, String> {
    let generated = generate(root)?;
    output::verify(&generated.files)?;
    Ok(generated.summary)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{generate, run_verify};

    fn workspace_root() -> &'static Path {
        Path::new("../..")
    }

    #[test]
    fn generation_is_deterministic() {
        let first = generate(workspace_root()).expect("first generation");
        let second = generate(workspace_root()).expect("second generation");
        assert_eq!(first.files.len(), second.files.len());
        for ((path_a, a), (path_b, b)) in first.files.iter().zip(second.files.iter()) {
            assert_eq!(path_a, path_b);
            assert_eq!(a, b, "generated output differs for {}", path_a.display());
        }
    }

    #[test]
    fn committed_sources_are_fresh() {
        run_verify(workspace_root()).expect("committed generated sources are fresh");
    }

    #[test]
    fn wrapper_metadata_is_emitted() {
        let generated = generate(workspace_root()).expect("generation");
        let tlprofile = generated
            .files
            .iter()
            .find(|(path, _)| path.to_string_lossy().contains("rtd-tlprofile"))
            .map(|(_, source)| source)
            .expect("rtd-tlprofile source is generated");
        assert!(tlprofile.contains("fn wrapper_metadata"));
        assert!(tlprofile.contains("WrapperKind::InitConnection"));
        assert!(tlprofile.contains("WrapperKind::InvokeAfterMsg"));
        assert!(tlprofile.contains("WrapperKind::InvokeAfterMsgs"));
        assert!(tlprofile.contains("WrapperKind::Layer"));
        assert!(tlprofile.contains("device_model: value.device_model.clone()"));
        assert!(tlprofile.contains("ordering: OrderingKey {"));
    }
}
