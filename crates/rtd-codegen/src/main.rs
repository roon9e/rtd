//! Multi-layer TL code generator tool.
#![warn(missing_docs)]
//!
//! Subcommands:
//!
//! - `generate` — regenerate the committed `rtd-tg`, `rtd-tlprofile` and
//!   `rtd-mt` sources.
//! - `verify` — regenerate in memory and fail if the committed files differ.
//! - `import` — copy schemas from a source directory and verify digests.
//! - `debug` — print a short diagnostic summary of the manifest universe.
//!
mod digest;
mod emit;
mod emit_mt;
mod emit_profile;
mod emit_tg;
mod format;
mod generate;
mod import;
mod json;
mod load;
mod model;
mod obligations;
mod output;
mod parse;
mod routes;
mod schema;
mod universe;

use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let root = workspace_root();
    let result = match command {
        "generate" => generate::run_generate(&root).map(|summary| println!("{summary}")),
        "verify" => generate::run_verify(&root).map(|summary| println!("{summary}")),
        "import" => match args.get(1) {
            Some(source) => {
                import::run(&root, &PathBuf::from(source)).map(|summary| println!("{summary}"))
            }
            None => Err("usage: rtd-codegen import <source-dir>".to_owned()),
        },
        "debug" => debug(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn print_help() {
    println!("rtd-codegen <generate|verify|import|debug>");
}

/// Finds the workspace root by walking up from the current directory until a
/// `schema/rtd.json` marker is found.
fn workspace_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join("schema/rtd.json").is_file() {
            return dir;
        }
        if !dir.pop() {
            return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }
    }
}

fn default_manifest() -> PathBuf {
    PathBuf::from("schema/layers/manifest.json")
}

/// Prints a diagnostic summary of the imported schemas and the plan built from
/// them. This is also what keeps the provenance and plan metadata in the
/// intermediate representation purposeful rather than dead.
fn debug() -> Result<(), String> {
    let manifest_path = default_manifest();
    let (universe, manifest) = load::load_universe(&manifest_path)?;
    let policy = schema::load_policy(&manifest_path.with_file_name("policy.json"))?;
    let config = schema::load_rtd_config(&PathBuf::from("schema/rtd.json"))?;

    println!(
        "manifest canonical={} repo={} source_path={} layers={} overlays={} legacy_methods={}",
        manifest.canonical_layer,
        manifest.repository,
        manifest.source_path,
        manifest.layers.len(),
        manifest.overlays.len(),
        manifest.legacy_methods.len(),
    );
    for schema in universe.schemas.values() {
        let source = &schema.source;
        println!(
            "schema layer={} file={} repo={} commit={} blob={} path={} source_layer={}",
            schema.layer,
            source.file,
            source.repository,
            source.commit,
            source.blob,
            source.path,
            source.layer,
        );
    }
    for overlay in &manifest.overlays {
        println!(
            "overlay file={} repo={} commit={} blob={} path={}",
            overlay.file, overlay.repository, overlay.commit, overlay.blob, overlay.path,
        );
    }

    let notes = policy
        .entries
        .iter()
        .filter(|entry| !entry.resolution.note.is_empty())
        .count();
    println!(
        "policy entries={} with_notes={}",
        policy.entries.len(),
        notes
    );

    let methods = universe
        .families
        .values()
        .filter(|family| family.is_method())
        .count();
    println!(
        "universe canonical={} layers={:?} families={} methods={} types={} wire_codecs={}",
        universe.canonical_layer,
        universe.layers(),
        universe.families.len(),
        methods,
        universe.families.len() - methods,
        universe.wire_codecs.len(),
    );
    for codec in universe.wire_codecs.values().take(3) {
        println!(
            "wire_codec id={:#010x} semantic={} layers={:?}",
            codec.wire_id,
            codec.key.as_string(),
            codec.layers,
        );
    }
    for family in universe.families.values() {
        for variant in &family.variants {
            println!(
                "variant semantic={} wire_id={:#010x} body={:?} layers={:?}",
                family.key.as_string(),
                variant.wire_id,
                variant.body_shape,
                variant.layers,
            );
        }
    }
    if let Some((key, _)) = universe.families.iter().next()
        && let Some(definition) = universe.definition(universe.canonical_layer, key)
    {
        println!(
            "sample_definition semantic={} wire_id={:#010x}",
            key.as_string(),
            definition.wire_id,
        );
    }

    let conversion = obligations::analyze(&universe, &policy)?;
    println!(
        "conversion canonical_layer={} obligations={} unresolved={}",
        conversion.canonical_layer,
        conversion.obligations.len(),
        conversion.unresolved().len(),
    );
    for obligation in conversion.unresolved().iter().take(20) {
        println!("UNRESOLVED {}", obligation.summary());
    }
    for profile in conversion.profiles.values() {
        let changed = profile
            .families
            .values()
            .filter(|family| family.body_changed)
            .count();
        println!(
            "profile layer={} families={} body_changed={}",
            profile.layer,
            profile.families.len(),
            changed,
        );
        for family in profile
            .families
            .values()
            .filter(|family| family.body_changed)
            .take(5)
        {
            println!("  changed {}", family.key.as_string());
        }
    }

    let routes = routes::build_route_table(&universe, &conversion, &config.enabled_layers)?;
    let mut modes: BTreeMap<&'static str, usize> = BTreeMap::new();
    for route in &routes.routes {
        *modes.entry(route.mode.as_str()).or_default() += 1;
    }
    println!(
        "routes profiles={:?} total={} modes={:?}",
        routes.profiles,
        routes.routes.len(),
        modes,
    );
    for route in routes.routes.iter().take(5) {
        match routes.wire_id(route.layer, &route.semantic) {
            Some(wire_id) => println!(
                "  route layer={} semantic={} wire_id={wire_id:#010x} mode={}",
                route.layer,
                route.semantic.as_string(),
                route.mode.as_str(),
            ),
            None => println!(
                "  route layer={} semantic={} wire_id=none mode={}",
                route.layer,
                route.semantic.as_string(),
                route.mode.as_str(),
            ),
        }
    }

    let plans = routes::build_plans(&universe, &conversion, &config.enabled_layers)?;
    println!(
        "plans body={} preflight={} result={}",
        plans.body_count(),
        plans.preflight_count(),
        plans.result_count(),
    );
    for plan in plans.body.iter().take(5) {
        println!(
            "  body_plan id={} digest={} routes={}",
            plan.id,
            routes::digest_hex(&plan.digest),
            plan.routes.len(),
        );
    }
    Ok(())
}
