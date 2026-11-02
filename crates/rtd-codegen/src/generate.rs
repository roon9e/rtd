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
    check_hook_registry(&plan, rtd_layerhooks_registry::HOOKS)?;
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

/// Enforces the hook coverage gate over the reviewed hook-backed obligations.
///
/// Every present-in-both obligation whose action requires a hook (`adapter` or
/// `project`) must name a registered hook whose direction covers the obligation,
/// and every registered hook must be referenced by at least one such obligation
/// (no stale signature). Hooks whose family exists only in an older layer
/// (`old-only`) are deferred to the follow-up `add-legacy-layer-constructors`
/// change and are not registered.
///
/// # Errors
///
/// Returns a diagnostic naming the first missing, mis-directed or stale hook.
fn check_hook_registry(
    plan: &obligations::ConversionPlan,
    hooks: &[rtd_layerhooks_registry::HookSpec],
) -> Result<(), String> {
    use std::collections::BTreeSet;

    let find = |name: &str| hooks.iter().find(|spec| spec.name == name);
    let mut referenced: BTreeSet<&str> = BTreeSet::new();
    for obligation in &plan.obligations {
        if obligation.kind == "old-only" {
            continue;
        }
        let Some(resolution) = &obligation.resolution else {
            continue;
        };
        if !matches!(resolution.action.as_str(), "adapter" | "project") {
            continue;
        }
        let spec = find(&resolution.hook).ok_or_else(|| {
            format!(
                "E_MISSING_HOOK_SIGNATURE: {} resolves to unregistered hook {:?}",
                obligation.summary(),
                resolution.hook
            )
        })?;
        if !spec.direction.covers(obligation.direction) {
            return Err(format!(
                "E_HOOK_DIRECTION: {} names hook {:?}, which does not cover direction {}",
                obligation.summary(),
                spec.name,
                obligation.direction
            ));
        }
        if spec.status == rtd_layerhooks_registry::Status::Pending {
            return Err(format!(
                "E_UNIMPLEMENTED_HOOK: {} names hook {:?}, which is registered but not \
                 yet implemented",
                obligation.summary(),
                spec.name
            ));
        }
        referenced.insert(spec.name);
    }
    for spec in hooks {
        if !referenced.contains(spec.name) {
            return Err(format!(
                "E_STALE_HOOK_SIGNATURE: registered hook {:?} is referenced by no \
                 present-in-both adapter obligation",
                spec.name
            ));
        }
    }
    Ok(())
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

    use super::{check_hook_registry, generate, run_verify};

    fn workspace_root() -> &'static Path {
        Path::new("../..")
    }

    /// The reviewed conversion plan for the enabled set.
    fn reviewed_plan() -> crate::obligations::ConversionPlan {
        let root = workspace_root();
        let (universe, _) =
            crate::load::load_universe(&root.join("schema/layers/manifest.json")).unwrap();
        let policy = crate::schema::load_policy(&root.join("schema/layers/policy.json")).unwrap();
        crate::obligations::analyze(&universe, &policy).unwrap()
    }

    #[test]
    fn hook_registry_covers_every_present_in_both_hook_backed_obligation() {
        let plan = reviewed_plan();
        check_hook_registry(&plan, rtd_layerhooks_registry::HOOKS)
            .expect("every present-in-both hook-backed obligation has a registered hook");
    }

    #[test]
    fn hook_registry_fails_on_a_missing_signature() {
        let plan = reviewed_plan();
        let error = check_hook_registry(&plan, &[]).unwrap_err();
        assert!(error.contains("E_MISSING_HOOK_SIGNATURE"), "{error}");
    }

    #[test]
    fn hook_registry_fails_on_a_stale_entry() {
        use rtd_layerhooks_registry::{Direction, HookSpec, Status};

        let plan = reviewed_plan();
        let mut hooks = rtd_layerhooks_registry::HOOKS.to_vec();
        hooks.push(HookSpec {
            name: "layerStaleHook",
            direction: Direction::Both,
            status: Status::Pending,
            signature: "",
        });
        let error = check_hook_registry(&plan, &hooks).unwrap_err();
        assert!(error.contains("E_STALE_HOOK_SIGNATURE"), "{error}");
    }

    #[test]
    fn hook_registry_fails_on_a_direction_mismatch() {
        use rtd_layerhooks_registry::Direction;

        let plan = reviewed_plan();
        let mut hooks = rtd_layerhooks_registry::HOOKS.to_vec();
        let spec = hooks
            .iter_mut()
            .find(|spec| spec.name == "layerRequireLegacyEphemeralSendPeer")
            .expect("ephemeral send hook is registered");
        // The obligation is canonical-to-profile; reversing the direction must
        // be rejected.
        spec.direction = Direction::ProfileToCanonical;
        let error = check_hook_registry(&plan, &hooks).unwrap_err();
        assert!(error.contains("E_HOOK_DIRECTION"), "{error}");
    }

    #[test]
    fn hook_registry_fails_on_an_unimplemented_hook() {
        use rtd_layerhooks_registry::Status;

        let plan = reviewed_plan();
        let mut hooks = rtd_layerhooks_registry::HOOKS.to_vec();
        let spec = hooks
            .iter_mut()
            .find(|spec| spec.name == "layerBotVerificationText")
            .expect("text hook is registered");
        spec.status = Status::Pending;
        let error = check_hook_registry(&plan, &hooks).unwrap_err();
        assert!(error.contains("E_UNIMPLEMENTED_HOOK"), "{error}");
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

    /// The rewrite emitter relies on an invariant of the enabled layer set:
    /// every present-in-both rewrite has a local body identical to canonical
    /// (same value fields, same order, same body shape) and differs only in the
    /// nested graph. So a rewrite encoder only has to route nested constructor
    /// ids through the profile — no field reordering, removal or flag changes.
    #[test]
    fn rewrites_are_nested_dirty_only() {
        use crate::model::FieldKind;
        use crate::obligations;
        use crate::routes::{RouteMode, build_route_table};

        let root = workspace_root();
        let (universe, _) =
            crate::load::load_universe(&root.join("schema/layers/manifest.json")).unwrap();
        let policy = crate::schema::load_policy(&root.join("schema/layers/policy.json")).unwrap();
        let config = crate::schema::load_rtd_config(&root.join("schema/rtd.json")).unwrap();
        let plan = obligations::analyze(&universe, &policy).unwrap();
        let table = build_route_table(&universe, &plan, &config.enabled_layers).unwrap();

        let mut rewrites = 0usize;
        for route in &table.routes {
            if route.mode != RouteMode::Rewrite {
                continue;
            }
            rewrites += 1;
            let profile = plan.profile(route.layer).unwrap();
            let conv = profile.family(&route.semantic).unwrap();
            let (canonical, profile_def) = (conv.canonical.as_ref(), conv.profile.as_ref());
            let (Some(canonical), Some(profile_def)) = (canonical, profile_def) else {
                panic!("rewrite {} has a missing side", route.semantic.as_string());
            };
            let names = |definition: &crate::model::Definition| -> Vec<String> {
                definition
                    .fields
                    .iter()
                    .filter(|field| field.kind == FieldKind::Value)
                    .map(|field| field.name.clone())
                    .collect()
            };
            assert_eq!(
                names(canonical),
                names(profile_def),
                "rewrite {} reorders or changes fields",
                route.semantic.as_string()
            );
            assert_eq!(
                canonical.body_shape,
                profile_def.body_shape,
                "rewrite {} has a differing local body",
                route.semantic.as_string()
            );
        }
        assert!(rewrites > 0, "expected at least one rewrite route");
    }

    /// The policy emitter cannot reuse the rewrite walker: a policy family's
    /// profile field set is usually a *reduced* view of canonical (fields are
    /// dropped for the older layer), so the emitter must walk the target layer's
    /// fields and pull from canonical, apply `default`s, reject on
    /// `reject`/`reject-if-present`, and call hooks for `adapter` obligations.
    #[test]
    fn policy_families_are_mostly_reduced_field_sets() {
        use crate::model::FieldKind;
        use crate::obligations;
        use crate::routes::{RouteMode, build_route_table};
        use std::collections::BTreeSet;

        let root = workspace_root();
        let (universe, _) =
            crate::load::load_universe(&root.join("schema/layers/manifest.json")).unwrap();
        let policy = crate::schema::load_policy(&root.join("schema/layers/policy.json")).unwrap();
        let config = crate::schema::load_rtd_config(&root.join("schema/rtd.json")).unwrap();
        let plan = obligations::analyze(&universe, &policy).unwrap();
        let table = build_route_table(&universe, &plan, &config.enabled_layers).unwrap();

        let mut same = 0usize;
        let mut differ = 0usize;
        let mut actions: BTreeSet<String> = BTreeSet::new();
        for route in &table.routes {
            if route.mode != RouteMode::Policy {
                continue;
            }
            let profile = plan.profile(route.layer).unwrap();
            let conv = profile.family(&route.semantic).unwrap();
            for obligation in conv.body_obligations() {
                if let Some(resolution) = &obligation.resolution {
                    actions.insert(resolution.action.clone());
                }
            }
            let (Some(canonical), Some(profile_def)) = (&conv.canonical, &conv.profile) else {
                continue;
            };
            let names = |definition: &crate::model::Definition| -> Vec<String> {
                definition
                    .fields
                    .iter()
                    .filter(|field| field.kind == FieldKind::Value)
                    .map(|field| field.name.clone())
                    .collect()
            };
            if names(canonical) == names(profile_def) {
                same += 1;
            } else {
                differ += 1;
            }
        }
        assert!(differ > same, "most policy families reduce the field set");
        assert!(actions.contains("drop"));
        assert!(actions.contains("adapter"));
        assert!(actions.contains("reject-if-present"));
    }

    /// Route completeness: every definition present in an enabled layer has an
    /// explicit `(profile, wire id)` route, so no represented pair is silently
    /// absent (a definition the layer lacks is deliberately unrouted, which is
    /// its explicit "unavailable" action).
    #[test]
    fn every_present_definition_has_a_route_in_its_layer() {
        use super::obligations;
        use crate::routes::build_route_table;

        let root = workspace_root();
        let (universe, _) =
            crate::load::load_universe(&root.join("schema/layers/manifest.json")).unwrap();
        let policy = crate::schema::load_policy(&root.join("schema/layers/policy.json")).unwrap();
        let config = crate::schema::load_rtd_config(&root.join("schema/rtd.json")).unwrap();
        let plan = obligations::analyze(&universe, &policy).unwrap();
        let table = build_route_table(&universe, &plan, &config.enabled_layers).unwrap();

        for &layer in &config.enabled_layers {
            let schema = &universe.schemas[&layer];
            for definition in &schema.definitions {
                assert!(
                    table.lookup(layer, definition.wire_id).is_some(),
                    "layer {layer} definition {} wire {:#010x} has no route",
                    definition.key.as_string(),
                    definition.wire_id
                );
            }
        }
    }

    /// Every registered `old-only` hook is a `ProfileOnly` button route, and the
    /// only `ProfileOnly` constructor without one is the passkey credential.
    #[test]
    fn legacy_hooks_cover_the_profile_only_buttons() {
        use super::obligations;
        use crate::routes::build_route_table;
        use std::collections::BTreeSet;

        let root = workspace_root();
        let (universe, _) =
            crate::load::load_universe(&root.join("schema/layers/manifest.json")).unwrap();
        let policy = crate::schema::load_policy(&root.join("schema/layers/policy.json")).unwrap();
        let config = crate::schema::load_rtd_config(&root.join("schema/rtd.json")).unwrap();
        let plan = obligations::analyze(&universe, &policy).unwrap();
        let table = build_route_table(&universe, &plan, &config.enabled_layers).unwrap();

        let profile_only: BTreeSet<u32> = table
            .routes
            .iter()
            .filter(|route| route.mode.as_str() == "profile-only")
            .map(|route| route.wire_id)
            .collect();
        let registered: BTreeSet<u32> = rtd_layerhooks_registry::LEGACY_HOOKS
            .iter()
            .map(|hook| hook.wire_id)
            .collect();

        for wire in &registered {
            assert!(
                profile_only.contains(wire),
                "registered legacy hook {wire:#010x} is not a profile-only route"
            );
        }
        let unregistered: BTreeSet<u32> = profile_only.difference(&registered).copied().collect();
        assert_eq!(
            unregistered,
            BTreeSet::from([0x5b1c_cb28]),
            "only inputPasskeyCredentialFirebasePNV may lack a legacy hook"
        );
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
