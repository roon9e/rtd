//! Assembles a validated [`Universe`] from a manifest, overlays and promoted
//! legacy methods, verifying every schema's SHA-256.

use std::path::Path;

use crate::model::{SourceRef, build_schema};
use crate::parse::{Category, RawDefinition, RawSchema, parse_schema};
use crate::schema::{self, Manifest};
use crate::universe::Universe;

/// A promoted legacy method parsed from the manifest.
struct LegacyMethod {
    name: String,
    namespace: Vec<String>,
    local: String,
    source: String,
    definition: RawDefinition,
}

/// Loads the full manifest universe rooted at the manifest's directory.
///
/// # Errors
///
/// Returns a message on a digest mismatch, a parse error or an inconsistent
/// overlay/legacy declaration.
pub(crate) fn load_universe(manifest_path: &Path) -> Result<(Universe, Manifest), String> {
    let manifest = schema::load_manifest(manifest_path)?;
    let root = manifest_path
        .parent()
        .ok_or_else(|| "manifest has no parent directory".to_owned())?;
    let legacy = parse_legacy(&manifest)?;

    let mut overlays = Vec::new();
    for entry in &manifest.overlays {
        let path = schema::resolve(root, &entry.file);
        let bytes = schema::verify_sha256(&path, &entry.sha256)?;
        let text = decode(&bytes, &path)?;
        let parsed = parse_schema(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        overlays.push(parsed);
    }

    let mut schemas = Vec::new();
    let mut legacy_matched = vec![false; legacy.len()];
    for entry in &manifest.layers {
        let path = schema::resolve(root, &entry.file);
        let bytes = schema::verify_sha256(&path, &entry.sha256)?;
        let text = decode(&bytes, &path)?;
        let mut parsed =
            parse_schema(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        for overlay in &overlays {
            merge(&mut parsed, overlay)
                .map_err(|error| format!("layer {} overlay: {error}", entry.layer))?;
        }
        let canonical = entry.layer == manifest.canonical_layer;
        apply_legacy(
            &mut parsed,
            entry.layer,
            canonical,
            &legacy,
            &mut legacy_matched,
        )
        .map_err(|error| format!("layer {} legacy methods: {error}", entry.layer))?;
        let source = SourceRef {
            layer: entry.layer,
            repository: manifest.repository.clone(),
            commit: entry.commit.clone(),
            blob: entry.blob.clone(),
            path: manifest.source_path.clone(),
            file: entry.file.clone(),
            sha256: crate::digest::sha256(&bytes),
        };
        schemas.push(build_schema(&parsed, source).map_err(|error| error.0)?);
    }
    for (index, matched) in legacy_matched.iter().enumerate() {
        if !matched {
            return Err(format!(
                "stale legacy method {:?}: no historical layer defines its wire id",
                legacy[index].name
            ));
        }
    }
    let universe = Universe::new(manifest.canonical_layer, schemas)?;
    Ok((universe, manifest))
}

fn decode(bytes: &[u8], path: &Path) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| format!("{} is not UTF-8", path.display()))
}

fn merge(target: &mut RawSchema, overlay: &RawSchema) -> Result<(), String> {
    for definition in &overlay.definitions {
        if let Some(existing) = target
            .definitions
            .iter()
            .find(|candidate| candidate.id == definition.id)
        {
            if existing == definition {
                continue;
            }
            return Err(format!(
                "E_OVERLAY_COLLISION: wire id {:#010x} conflicts",
                definition.id
            ));
        }
        target.definitions.push(definition.clone());
    }
    if target.layer == 0 {
        target.layer = overlay.layer;
    }
    Ok(())
}

fn parse_legacy(manifest: &Manifest) -> Result<Vec<LegacyMethod>, String> {
    let mut result = Vec::new();
    for entry in &manifest.legacy_methods {
        let text = format!("---functions---\n{}\n", entry.definition);
        let parsed = parse_schema(&text).map_err(|error| format!("legacy method: {error}"))?;
        if parsed.definitions.len() != 1 || parsed.definitions[0].category != Category::Function {
            return Err(format!(
                "legacy method {:?} must be exactly one function",
                entry.name
            ));
        }
        let definition = parsed.definitions[0].clone();
        if !definition.generic_params.is_empty() {
            return Err(format!(
                "legacy method {:?} has generic parameters",
                entry.name
            ));
        }
        let source = definition.qualified();
        let (namespace, local) = match entry.name.rsplit_once('.') {
            Some((namespace, local)) => (
                namespace.split('.').map(str::to_owned).collect::<Vec<_>>(),
                local.to_owned(),
            ),
            None => (Vec::new(), entry.name.clone()),
        };
        if local.is_empty() || namespace != definition.namespace {
            return Err(format!(
                "legacy method {:?} must be an identifier in namespace {:?}",
                entry.name, definition.namespace
            ));
        }
        if entry.name == source {
            return Err(format!(
                "legacy method {:?} does not differ from its source",
                entry.name
            ));
        }
        result.push(LegacyMethod {
            name: entry.name.clone(),
            namespace,
            local,
            source,
            definition,
        });
    }
    Ok(result)
}

fn apply_legacy(
    schema: &mut RawSchema,
    layer: i32,
    canonical: bool,
    methods: &[LegacyMethod],
    matched: &mut [bool],
) -> Result<(), String> {
    for (index, method) in methods.iter().enumerate() {
        let wire_index = schema
            .definitions
            .iter()
            .position(|candidate| candidate.id == method.definition.id);
        let source_index = schema.definitions.iter().position(|candidate| {
            candidate.category == Category::Function && candidate.qualified() == method.source
        });
        if schema
            .definitions
            .iter()
            .any(|candidate| candidate.qualified() == method.name)
        {
            return Err(format!(
                "layer {layer} already defines legacy method {:?}",
                method.name
            ));
        }
        if canonical {
            if wire_index.is_some() {
                return Err(format!(
                    "canonical layer {layer} still defines legacy wire id {:#010x}",
                    method.definition.id
                ));
            }
            if source_index.is_none() {
                return Err(format!(
                    "canonical layer {layer} has no superseding method {:?}",
                    method.source
                ));
            }
            let mut promoted = method.definition.clone();
            promoted.namespace = method.namespace.clone();
            promoted.name = method.local.clone();
            schema.definitions.push(promoted);
            matched[index] = true;
            continue;
        }
        let Some(wire_index) = wire_index else {
            continue;
        };
        let official = &schema.definitions[wire_index];
        if official.category != Category::Function || official != &method.definition {
            return Err(format!(
                "layer {layer} wire id {:#010x} differs from reviewed legacy definition {:?}",
                method.definition.id, method.name
            ));
        }
        let mut renamed = official.clone();
        renamed.namespace = method.namespace.clone();
        renamed.name = method.local.clone();
        schema.definitions[wire_index] = renamed;
        matched[index] = true;
    }
    Ok(())
}
