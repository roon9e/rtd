//! Route classification over the full `(wire × profile)` product and the
//! digest-deduplicated execution plans.
//!
//! This mirrors `td/gen/layer_wire_model.go`'s `classifyLayerFamilyAction` and
//! `td/gen/layer_execution_plan.go`'s plan construction. Every `(profile, wire
//! id)` pair receives an explicit action; a missing pair is a generation error.

use std::collections::{BTreeMap, BTreeSet};

use crate::digest::{DigestWriter, to_hex};
use crate::model::{Definition, FieldKind, SchemaModel, SemanticKey, Shape, TypeKind};
use crate::obligations::{Availability, ConversionPlan, FamilyConversion};
use crate::parse::Category;
use crate::universe::Universe;

/// The six generated route modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteMode {
    /// Same wire id and complete nested graph.
    Direct,
    /// Different wire id, identical body.
    Retag,
    /// Body, projection or nested graph differs.
    Rewrite,
    /// An explicit policy decision applies.
    Policy,
    /// Present only in the profile.
    ProfileOnly,
    /// Absent or rejected.
    Unavailable,
}

impl RouteMode {
    /// The lowercase identifier used in generated tables.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            RouteMode::Direct => "direct",
            RouteMode::Retag => "retag",
            RouteMode::Rewrite => "rewrite",
            RouteMode::Policy => "policy",
            RouteMode::ProfileOnly => "profile-only",
            RouteMode::Unavailable => "unavailable",
        }
    }
}

/// One accepted `(profile, wire id)` route.
#[derive(Debug, Clone)]
pub(crate) struct Route {
    /// Profile layer.
    pub(crate) layer: i32,
    /// Wire identifier in that profile.
    pub(crate) wire_id: u32,
    /// Semantic key.
    pub(crate) semantic: SemanticKey,
    /// Semantic stable identity.
    pub(crate) semantic_id: u64,
    /// Classified mode.
    pub(crate) mode: RouteMode,
}

/// The complete route table for the enabled profiles.
#[derive(Debug, Clone, Default)]
pub(crate) struct RouteTable {
    /// Enabled profiles, ascending.
    pub(crate) profiles: Vec<i32>,
    /// Routes in deterministic order.
    pub(crate) routes: Vec<Route>,
}

impl RouteTable {
    /// The route for an exact `(layer, wire id)`, if accepted.
    pub(crate) fn lookup(&self, layer: i32, wire_id: u32) -> Option<&Route> {
        self.routes
            .iter()
            .find(|route| route.layer == layer && route.wire_id == wire_id)
    }

    /// The wire identifier for a `(layer, semantic)` pair.
    pub(crate) fn wire_id(&self, layer: i32, semantic: &SemanticKey) -> Option<u32> {
        self.routes
            .iter()
            .find(|route| route.layer == layer && &route.semantic == semantic)
            .map(|route| route.wire_id)
    }

    /// A stable per-mode route count for the generation summary.
    pub(crate) fn mode_summary(&self) -> String {
        let mut counts = [0usize; 6];
        for route in &self.routes {
            counts[mode_for(route.mode) as usize] += 1;
        }
        format!(
            "direct={} retag={} rewrite={} unavailable={} policy={} profile_only={}",
            counts[0], counts[1], counts[2], counts[3], counts[4], counts[5],
        )
    }
}

/// Classifies one family conversion, mirroring `classifyLayerFamilyAction`.
pub(crate) fn classify(conversion: &FamilyConversion, nested_dirty: bool) -> RouteMode {
    match conversion.availability {
        Availability::Absent | Availability::CanonicalOnly => RouteMode::Unavailable,
        Availability::ProfileOnly => RouteMode::ProfileOnly,
        Availability::Present => {
            if !conversion.body_obligations().is_empty() {
                return RouteMode::Policy;
            }
            let canonical = conversion.canonical.as_ref().expect("present canonical");
            let profile = conversion.profile.as_ref().expect("present profile");
            let same_body = canonical.body_shape == profile.body_shape;
            let same_id = canonical.wire_id == profile.wire_id;
            match (same_body, same_id, nested_dirty) {
                (true, true, false) => RouteMode::Direct,
                (true, false, false) => RouteMode::Retag,
                _ => RouteMode::Rewrite,
            }
        }
    }
}

/// Computes transitive "nested dirty" flags per family for a profile.
///
/// A family is nested-dirty when any type it references resolves to a family
/// whose body differs in the profile.
fn nested_dirty(
    universe: &Universe,
    plan: &ConversionPlan,
    layer: i32,
) -> BTreeMap<SemanticKey, bool> {
    let Some(profile) = plan.profile(layer) else {
        return BTreeMap::new();
    };
    let schema = &universe.schemas[&layer];
    let mut own: BTreeMap<SemanticKey, bool> = BTreeMap::new();
    for (key, conversion) in &profile.families {
        let changed = match conversion.availability {
            Availability::Present => conversion.body_changed,
            Availability::ProfileOnly | Availability::CanonicalOnly => true,
            Availability::Absent => false,
        };
        own.insert(key.clone(), changed);
    }
    let mut result = BTreeMap::new();
    for key in profile.families.keys() {
        let Some(definition) = schema.find_key(key) else {
            result.insert(key.clone(), false);
            continue;
        };
        let mut stack = Vec::new();
        let mut nested = false;
        collect_dependencies(schema, definition, &mut stack);
        while let Some(dependency) = stack.pop() {
            if own.get(&dependency).copied().unwrap_or(false) {
                nested = true;
                break;
            }
        }
        result.insert(key.clone(), nested);
    }
    result
}

fn collect_dependencies(schema: &SchemaModel, definition: &Definition, out: &mut Vec<SemanticKey>) {
    for field in &definition.fields {
        if field.kind == FieldKind::Value {
            collect_ref_dependencies(schema, &field.ty, out);
        }
    }
}

fn collect_ref_dependencies(
    schema: &SchemaModel,
    reference: &crate::model::TypeRef,
    out: &mut Vec<SemanticKey>,
) {
    match reference.kind {
        TypeKind::Vector => {
            if let Some(arg) = &reference.arg {
                collect_ref_dependencies(schema, arg, out);
            }
        }
        TypeKind::Named => {
            if reference.bare || reference.percent {
                let key = SemanticKey {
                    category: Category::Type,
                    qname: reference.qname.clone(),
                };
                if schema.find_key(&key).is_some() {
                    out.push(key);
                }
                return;
            }
            let constructors = schema.class_constructors(&reference.qname);
            for constructor in constructors {
                out.push(constructor.key.clone());
            }
        }
        TypeKind::GenericRef => {
            // Generic slots may contain any constructor.
            for definition in &schema.definitions {
                out.push(definition.key.clone());
            }
        }
        TypeKind::Primitive => {
            if reference.qname == "Object" {
                for definition in &schema.definitions {
                    out.push(definition.key.clone());
                }
            }
        }
    }
}

/// Builds the route table for the enabled profiles with a completeness check.
///
/// # Errors
///
/// Returns a message when a `(profile, wire id)` pair has no explicit action or
/// when a present definition is not represented by a route.
pub(crate) fn build_route_table(
    universe: &Universe,
    plan: &ConversionPlan,
    enabled: &[i32],
) -> Result<RouteTable, String> {
    let mut profiles: Vec<i32> = enabled.to_vec();
    profiles.sort_unstable();
    profiles.dedup();
    for layer in &profiles {
        if !universe.schemas.contains_key(layer) {
            return Err(format!("enabled layer {layer} is absent from the manifest"));
        }
    }
    let dirty: BTreeMap<i32, BTreeMap<SemanticKey, bool>> = profiles
        .iter()
        .map(|layer| (*layer, nested_dirty(universe, plan, *layer)))
        .collect();

    let mut routes = Vec::new();
    let mut accepted: BTreeSet<(i32, u32)> = BTreeSet::new();
    for layer in &profiles {
        let profile = plan
            .profile(*layer)
            .ok_or_else(|| format!("conversion plan misses layer {layer}"))?;
        let schema = &universe.schemas[layer];
        // Every wire codec must receive an explicit action for this profile.
        for wire_id in universe.wire_codecs.keys() {
            let definition = schema.find_wire(*wire_id);
            match definition {
                None => {
                    // Explicitly unavailable: the absence is the action.
                }
                Some(definition) => {
                    let conversion = profile.family(&definition.key).ok_or_else(|| {
                        format!("layer {layer} wire {wire_id:#010x} has no family conversion")
                    })?;
                    let nested = dirty
                        .get(layer)
                        .and_then(|map| map.get(&definition.key))
                        .copied()
                        .unwrap_or(false);
                    let mode = classify(conversion, nested);
                    if !accepted.insert((*layer, *wire_id)) {
                        return Err(format!(
                            "duplicate route for layer {layer} wire {wire_id:#010x}"
                        ));
                    }
                    routes.push(Route {
                        layer: *layer,
                        wire_id: *wire_id,
                        semantic: definition.key.clone(),
                        semantic_id: crate::model::semantic_stable_id(&definition.key),
                        mode,
                    });
                }
            }
        }
        // Every present definition must be reachable through some wire route.
        for definition in &schema.definitions {
            let expected = accepted.contains(&(*layer, definition.wire_id));
            if !expected {
                return Err(format!(
                    "layer {layer} definition {} wire {:#010x} has no accepted action",
                    definition.key.as_string(),
                    definition.wire_id
                ));
            }
        }
    }
    routes.sort_by(|a, b| {
        a.layer
            .cmp(&b.layer)
            .then_with(|| a.wire_id.cmp(&b.wire_id))
    });
    Ok(RouteTable { profiles, routes })
}

/// One deduplicated execution plan.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    /// Stable plan index.
    pub(crate) id: usize,
    /// Plan digest.
    pub(crate) digest: Shape,
    /// Routes sharing this plan.
    pub(crate) routes: Vec<(i32, SemanticKey, u32, RouteMode)>,
}

/// The three deduplicated plan sets.
#[derive(Debug, Clone, Default)]
pub(crate) struct PlanSet {
    /// Body plans (non-direct routes only).
    pub(crate) body: Vec<Plan>,
    /// Preflight plans (one per method route).
    pub(crate) preflight: Vec<Plan>,
    /// Result plans (one per method route with a canonical variant).
    pub(crate) result: Vec<Plan>,
}

impl PlanSet {
    /// Number of emitted body units.
    pub(crate) fn body_count(&self) -> usize {
        self.body.len()
    }
    /// Number of emitted preflight units.
    pub(crate) fn preflight_count(&self) -> usize {
        self.preflight.len()
    }
    /// Number of emitted result units.
    pub(crate) fn result_count(&self) -> usize {
        self.result.len()
    }
}

fn mode_for(kind: RouteMode) -> u64 {
    match kind {
        RouteMode::Direct => 0,
        RouteMode::Retag => 1,
        RouteMode::Rewrite => 2,
        RouteMode::Unavailable => 3,
        RouteMode::Policy => 4,
        RouteMode::ProfileOnly => 5,
    }
}

fn write_conversion(writer: &mut DigestWriter, conversion: &FamilyConversion, result_only: bool) {
    writer.uint64(conversion.availability as u64);
    if !result_only {
        writer.uint64(conversion.fields.len() as u64);
        for field in &conversion.fields {
            writer.uint64(
                field
                    .canonical_ordinal
                    .map_or(0, |ordinal| ordinal as u64 + 1),
            );
            writer.uint64(field.profile_ordinal as u64 + 1);
            writer.string(&field.canonical_name);
            writer.string(&field.profile_name);
        }
    }
    writer.uint64(conversion.obligations.len() as u64);
    for obligation in &conversion.obligations {
        let is_result = obligation.kind == "result";
        if result_only != is_result {
            continue;
        }
        let resolution = obligation.resolution.clone().unwrap_or_default();
        writer.string(&obligation.key);
        writer.string(obligation.kind);
        writer.string(obligation.direction);
        writer.string(&resolution.action);
        writer.string(&resolution.hook);
        writer.string(&resolution.target);
    }
}

fn collect_type_dependencies(
    schema: &SchemaModel,
    reference: &crate::model::TypeRef,
    out: &mut BTreeSet<SemanticKey>,
) -> bool {
    match reference.kind {
        TypeKind::Vector => reference
            .arg
            .as_deref()
            .is_some_and(|arg| collect_type_dependencies(schema, arg, out)),
        TypeKind::Named => {
            if reference.bare || reference.percent {
                let key = SemanticKey {
                    category: Category::Type,
                    qname: reference.qname.clone(),
                };
                if schema.find_key(&key).is_some() {
                    out.insert(key);
                }
                return false;
            }
            let constructors = schema.class_constructors(&reference.qname);
            if constructors.is_empty() {
                let key = SemanticKey {
                    category: Category::Type,
                    qname: reference.qname.clone(),
                };
                if schema.find_key(&key).is_some() {
                    out.insert(key);
                }
                return false;
            }
            for constructor in constructors {
                out.insert(constructor.key.clone());
            }
            false
        }
        TypeKind::GenericRef => true,
        TypeKind::Primitive => reference.qname == "Object",
    }
}

fn transitive_graph_digest(schema: &SchemaModel, roots: &[SemanticKey]) -> Result<Shape, String> {
    let mut seen: BTreeSet<SemanticKey> = BTreeSet::new();
    let mut queue: Vec<SemanticKey> = roots.to_vec();
    let mut head = 0;
    while head < queue.len() {
        let key = queue[head].clone();
        head += 1;
        if seen.contains(&key) {
            continue;
        }
        let definition = schema.find_key(&key).ok_or_else(|| {
            format!(
                "definition {} absent from layer {}",
                key.as_string(),
                schema.layer
            )
        })?;
        seen.insert(key.clone());
        let mut dependencies = BTreeSet::new();
        let mut dynamic = false;
        for field in &definition.fields {
            if field.kind == FieldKind::Value {
                dynamic |= collect_type_dependencies(schema, &field.ty, &mut dependencies);
            }
        }
        if dynamic {
            for candidate in &schema.definitions {
                dependencies.insert(candidate.key.clone());
            }
        }
        queue.extend(dependencies);
    }
    let mut writer = DigestWriter::new("gotd.tlprofile.transitive-graph.v1");
    writer.uint64(seen.len() as u64);
    for key in &seen {
        let definition = schema.find_key(key).expect("seen definition exists");
        writer.string(&key.as_string());
        writer.uint64(u64::from(definition.wire_id));
        writer.bytes(&definition.wire_shape);
        writer.bytes(&definition.body_shape);
        let mut dependencies = BTreeSet::new();
        let mut dynamic = false;
        for field in &definition.fields {
            if field.kind == FieldKind::Value {
                dynamic |= collect_type_dependencies(schema, &field.ty, &mut dependencies);
            }
        }
        if dynamic {
            for candidate in &schema.definitions {
                dependencies.insert(candidate.key.clone());
            }
        }
        writer.uint64(dependencies.len() as u64);
        for dependency in &dependencies {
            writer.string(&dependency.as_string());
        }
    }
    Ok(writer.finish())
}

fn transitive_definition_digest(schema: &SchemaModel, root: &Definition) -> Result<Shape, String> {
    transitive_graph_digest(schema, std::slice::from_ref(&root.key))
}

fn transitive_type_ref_digest(
    schema: &SchemaModel,
    reference: &crate::model::TypeRef,
) -> Result<Shape, String> {
    let mut keys = BTreeSet::new();
    let dynamic = collect_type_dependencies(schema, reference, &mut keys);
    if dynamic {
        for definition in &schema.definitions {
            keys.insert(definition.key.clone());
        }
    }
    let ordered: Vec<SemanticKey> = keys.into_iter().collect();
    let mut writer = DigestWriter::new("gotd.tlprofile.typeref-closure.v1");
    writer.string(&reference.as_string());
    let digest = transitive_graph_digest(schema, &ordered)?;
    writer.raw(&digest);
    Ok(writer.finish())
}

fn body_digest(
    universe: &Universe,
    layer: i32,
    key: &SemanticKey,
    mode: RouteMode,
    conversion: &FamilyConversion,
) -> Result<Shape, String> {
    let mut writer = DigestWriter::new("gotd.tlprofile.body-plan.v1");
    writer.uint64(mode_for(mode));
    writer.string(&key.as_string());
    write_conversion(&mut writer, conversion, false);
    let schema = &universe.schemas[&layer];
    let profile = conversion.profile.as_ref().expect("body plan profile");
    let transitive = transitive_definition_digest(schema, profile)?;
    writer.raw(&transitive);
    Ok(writer.finish())
}

fn preflight_digest(
    universe: &Universe,
    layer: i32,
    key: &SemanticKey,
    conversion: &FamilyConversion,
) -> Result<Shape, String> {
    let mut writer = DigestWriter::new("gotd.tlprofile.preflight-plan.v1");
    writer.string(&key.as_string());
    let profile = conversion.profile.as_ref().expect("preflight profile");
    writer.bytes(&profile.wire_shape);
    let schema = &universe.schemas[&layer];
    let transitive = transitive_definition_digest(schema, profile)?;
    writer.raw(&transitive);
    Ok(writer.finish())
}

fn result_digest(
    universe: &Universe,
    layer: i32,
    key: &SemanticKey,
    conversion: &FamilyConversion,
) -> Result<Shape, String> {
    let mut writer = DigestWriter::new("gotd.tlprofile.result-plan.v1");
    let canonical = conversion.canonical.as_ref().expect("result canonical");
    let profile = conversion.profile.as_ref().expect("result profile");
    writer.string(&canonical.result.as_string());
    writer.string(&profile.result.as_string());
    if Universe::type_ref_dynamic(&canonical.result) || Universe::type_ref_dynamic(&profile.result)
    {
        writer.string(&key.as_string());
    }
    write_conversion(&mut writer, conversion, true);
    let canonical_schema = universe.canonical();
    let canonical_digest = transitive_type_ref_digest(canonical_schema, &canonical.result)?;
    let profile_schema = &universe.schemas[&layer];
    let profile_digest = transitive_type_ref_digest(profile_schema, &profile.result)?;
    writer.raw(&canonical_digest);
    writer.raw(&profile_digest);
    Ok(writer.finish())
}

fn materialize(drafts: BTreeMap<Shape, Vec<(i32, SemanticKey, u32, RouteMode)>>) -> Vec<Plan> {
    drafts
        .into_iter()
        .enumerate()
        .map(|(id, (digest, routes))| Plan { id, digest, routes })
        .collect()
}

/// Builds body/preflight/result plans, deduplicating by digest.
///
/// # Errors
///
/// Returns a message when a route is missing a required plan.
pub(crate) fn build_plans(
    universe: &Universe,
    plan: &ConversionPlan,
    enabled: &[i32],
) -> Result<PlanSet, String> {
    let mut body: BTreeMap<Shape, Vec<(i32, SemanticKey, u32, RouteMode)>> = BTreeMap::new();
    let mut preflight: BTreeMap<Shape, Vec<(i32, SemanticKey, u32, RouteMode)>> = BTreeMap::new();
    let mut result: BTreeMap<Shape, Vec<(i32, SemanticKey, u32, RouteMode)>> = BTreeMap::new();
    let dirty: BTreeMap<i32, BTreeMap<SemanticKey, bool>> = enabled
        .iter()
        .map(|layer| (*layer, nested_dirty(universe, plan, *layer)))
        .collect();

    for layer in enabled {
        let profile = plan
            .profile(*layer)
            .ok_or_else(|| format!("conversion plan misses layer {layer}"))?;
        for (key, conversion) in &profile.families {
            let Some(profile_definition) = &conversion.profile else {
                continue;
            };
            let nested = dirty
                .get(layer)
                .and_then(|map| map.get(key))
                .copied()
                .unwrap_or(false);
            let mode = classify(conversion, nested);
            let wire_id = profile_definition.wire_id;
            let route = (*layer, key.clone(), wire_id, mode);
            if mode != RouteMode::Direct {
                let digest = body_digest(universe, *layer, key, mode, conversion)?;
                body.entry(digest).or_default().push(route.clone());
            }
            if key.category == Category::Function {
                let digest = preflight_digest(universe, *layer, key, conversion)?;
                preflight.entry(digest).or_default().push(route.clone());
                if conversion.canonical.is_some() {
                    let digest = result_digest(universe, *layer, key, conversion)?;
                    result.entry(digest).or_default().push(route);
                }
            }
        }
    }
    Ok(PlanSet {
        body: materialize(body),
        preflight: materialize(preflight),
        result: materialize(result),
    })
}

/// Renders a plan digest as hex (used by diagnostics and tests).
pub(crate) fn digest_hex(digest: &Shape) -> String {
    to_hex(digest)
}

#[cfg(test)]
mod tests {
    use super::{RouteMode, build_plans, build_route_table, classify};
    use crate::model::{SemanticKey, SourceRef, build_schema};
    use crate::obligations::{Availability, FamilyConversion, analyze};
    use crate::parse::parse_schema;
    use crate::schema::Policy;
    use crate::universe::Universe;

    fn source(layer: i32) -> SourceRef {
        SourceRef {
            layer,
            repository: "r".to_owned(),
            commit: "c".to_owned(),
            blob: "b".to_owned(),
            path: "p".to_owned(),
            file: "f".to_owned(),
            sha256: [0; 32],
        }
    }

    fn universe(schemas: &[(i32, &str)]) -> Universe {
        let models = schemas
            .iter()
            .map(|(layer, text)| {
                build_schema(&parse_schema(text).unwrap(), source(*layer)).unwrap()
            })
            .collect();
        Universe::new(232, models).unwrap()
    }

    #[test]
    fn canonical_routes_are_direct() {
        let universe = universe(&[(232, "foo#1 a:int = Foo;\n")]);
        let plan = analyze(
            &universe,
            &Policy {
                entries: Vec::new(),
            },
        )
        .unwrap();
        let table = build_route_table(&universe, &plan, &[232]).unwrap();
        let route = table.lookup(232, 1).expect("route");
        assert_eq!(route.mode, RouteMode::Direct);
    }

    #[test]
    fn completeness_covers_every_pair() {
        let universe = universe(&[
            (231, "foo#1 a:int = Foo;\nbar#2 = Bar;\n"),
            (232, "foo#1 a:int = Foo;\n"),
        ]);
        let plan = analyze(
            &universe,
            &Policy {
                entries: Vec::new(),
            },
        )
        .unwrap();
        let table = build_route_table(&universe, &plan, &[231, 232]).unwrap();
        // Layer 231 has bar, canonical 232 has only foo.
        assert!(table.lookup(231, 2).is_some());
        assert!(table.lookup(232, 2).is_none());
    }

    #[test]
    fn classification_modes() {
        let mut conversion = FamilyConversion {
            key: SemanticKey {
                category: crate::parse::Category::Type,
                qname: "foo".to_owned(),
            },
            canonical: Some(
                build_schema(&parse_schema("foo#1 a:int = Foo;\n").unwrap(), source(232))
                    .unwrap()
                    .definitions[0]
                    .clone(),
            ),
            profile: Some(
                build_schema(&parse_schema("foo#2 a:int = Foo;\n").unwrap(), source(231))
                    .unwrap()
                    .definitions[0]
                    .clone(),
            ),
            availability: Availability::Present,
            body_changed: false,
            result_changed: false,
            fields: Vec::new(),
            obligations: Vec::new(),
        };
        assert_eq!(classify(&conversion, false), RouteMode::Retag);
        conversion.availability = Availability::ProfileOnly;
        assert_eq!(classify(&conversion, false), RouteMode::ProfileOnly);
        conversion.availability = Availability::CanonicalOnly;
        assert_eq!(classify(&conversion, false), RouteMode::Unavailable);
    }

    #[test]
    fn semantic_identity_is_stable_across_wire_ids() {
        use crate::model::semantic_stable_id;

        let universe = universe(&[(231, "foo#1 a:int = Foo;\n"), (232, "foo#2 a:int = Foo;\n")]);
        let key = SemanticKey {
            category: crate::parse::Category::Type,
            qname: "foo".to_owned(),
        };
        let family = universe.families.get(&key).unwrap();
        // The wire identifier differs between layers...
        assert_eq!(universe.definition(231, &key).unwrap().wire_id, 1);
        assert_eq!(universe.definition(232, &key).unwrap().wire_id, 2);
        // ...but the semantic identity is stable and independent of it.
        assert_eq!(family.stable_id(), semantic_stable_id(&key));
        assert_eq!(family.stable_id(), family.stable_id());
    }

    #[test]
    fn plans_deduplicate_by_digest() {
        // Two methods returning the same class result should share one result
        // plan, while each preflight plan is method-scoped.
        let universe = universe(&[(
            232,
            "r#3 x:int = Reply;\n---functions---\nm1#1 a:int = Reply;\nm2#2 b:string = Reply;\n",
        )]);
        let plan = analyze(
            &universe,
            &Policy {
                entries: Vec::new(),
            },
        )
        .unwrap();
        let plans = build_plans(&universe, &plan, &[232]).unwrap();
        assert_eq!(plans.result_count(), 1, "identical results collapse");
        assert_eq!(plans.preflight_count(), 2, "one preflight per method");
    }
}
