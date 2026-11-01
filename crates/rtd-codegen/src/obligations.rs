//! Conversion analysis and reviewed policy.
//!
//! A faithful port of `td/gen/layer_conversion.go` and
//! `td/gen/layer_obligations.go`. Every non-mechanical difference between a
//! profile and the canonical layer becomes an [`Obligation`] identified by a
//! domain-separated digest; generation fails on an unresolved obligation or a
//! policy entry that matches no difference.

use std::collections::BTreeMap;

use crate::digest::{DigestWriter, to_hex};
use crate::model::{Definition, Field, FieldKind, SemanticKey, Shape, TypeKind, TypeRef};
use crate::parse::Category;
use crate::schema::{Policy, Resolution};
use crate::universe::Universe;

/// Availability of a family in an exact profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Availability {
    /// Present in both canonical and profile.
    Present = 0,
    /// Present only in canonical.
    CanonicalOnly = 1,
    /// Present only in the profile.
    ProfileOnly = 2,
    /// Absent on both sides.
    Absent = 3,
}

/// A canonical field paired with its profile field.
#[derive(Debug, Clone)]
pub(crate) struct FieldMapping {
    /// Canonical ordinal, or empty.
    pub(crate) canonical_ordinal: Option<usize>,
    /// Profile ordinal.
    pub(crate) profile_ordinal: usize,
    /// Canonical field name, or empty.
    pub(crate) canonical_name: String,
    /// Profile field name.
    pub(crate) profile_name: String,
}

/// One generated compatibility decision.
#[derive(Debug, Clone)]
pub(crate) struct Obligation {
    /// Domain-separated key.
    pub(crate) key: String,
    /// Stable kind identifier.
    pub(crate) kind: &'static str,
    /// Layer the decision applies to.
    pub(crate) layer: i32,
    /// Conversion direction.
    pub(crate) direction: &'static str,
    /// Canonical semantic key.
    pub(crate) semantic: SemanticKey,
    /// Other-side semantic key.
    pub(crate) other_semantic: SemanticKey,
    /// Canonical wire identifier.
    pub(crate) wire_id: u32,
    /// Other-side wire identifier.
    pub(crate) other_wire_id: u32,
    /// Canonical field name.
    pub(crate) field: String,
    /// Other-side field name.
    pub(crate) other_field: String,
    /// Flags word.
    pub(crate) flag_word: String,
    /// Flags bit.
    pub(crate) flag_bit: u8,
    /// Participating fields.
    pub(crate) fields: Vec<String>,
    /// Source type spelling.
    pub(crate) source_type: String,
    /// Target type spelling.
    pub(crate) target_type: String,
    /// Source signature shape.
    pub(crate) source_shape: Shape,
    /// Target signature shape.
    pub(crate) target_shape: Shape,
    /// Applied resolution, if any.
    pub(crate) resolution: Option<Resolution>,
}

impl Obligation {
    /// A human-readable one-line summary for diagnostics.
    pub(crate) fn summary(&self) -> String {
        format!(
            "{} kind={} layer={} {} semantic={} wire={:#010x}",
            self.key,
            self.kind,
            self.layer,
            self.direction,
            self.semantic.as_string(),
            self.wire_id
        )
    }
}

/// The conversion decision for one family in one profile.
#[derive(Debug, Clone)]
pub(crate) struct FamilyConversion {
    /// Semantic key.
    pub(crate) key: SemanticKey,
    /// Canonical definition, if present.
    pub(crate) canonical: Option<Definition>,
    /// Profile definition, if present.
    pub(crate) profile: Option<Definition>,
    /// Availability.
    pub(crate) availability: Availability,
    /// Whether the body or wire identifier differs.
    pub(crate) body_changed: bool,
    /// Whether the RPC result differs.
    pub(crate) result_changed: bool,
    /// Canonical-to-profile field mapping.
    pub(crate) fields: Vec<FieldMapping>,
    /// Obligations attached to this family.
    pub(crate) obligations: Vec<Obligation>,
}

impl FamilyConversion {
    /// Body-affecting obligations (everything except result/update/new-only).
    pub(crate) fn body_obligations(&self) -> Vec<&Obligation> {
        self.obligations
            .iter()
            .filter(|obligation| {
                !matches!(obligation.kind, "result" | "new-only" | "update-projection")
            })
            .collect()
    }
}

/// One profile's conversion decisions.
#[derive(Debug, Clone)]
pub(crate) struct ProfileConversion {
    /// Layer number.
    pub(crate) layer: i32,
    /// Families keyed by semantic key.
    pub(crate) families: BTreeMap<SemanticKey, FamilyConversion>,
}

impl ProfileConversion {
    /// Looks up a family conversion.
    pub(crate) fn family(&self, key: &SemanticKey) -> Option<&FamilyConversion> {
        self.families.get(key)
    }
}

/// The complete conversion plan.
#[derive(Debug, Clone)]
pub(crate) struct ConversionPlan {
    /// Canonical layer.
    pub(crate) canonical_layer: i32,
    /// Profiles keyed by layer.
    pub(crate) profiles: BTreeMap<i32, ProfileConversion>,
    /// All generated obligations.
    pub(crate) obligations: Vec<Obligation>,
}

impl ConversionPlan {
    /// Looks up a profile conversion.
    pub(crate) fn profile(&self, layer: i32) -> Option<&ProfileConversion> {
        self.profiles.get(&layer)
    }

    /// Obligations with no resolved action.
    pub(crate) fn unresolved(&self) -> Vec<&Obligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.resolution.is_none())
            .collect()
    }
}

const KIND_ALIAS: &str = "alias";
const KIND_REQUIRED: &str = "required";
const KIND_INCOMPATIBLE: &str = "incompatible";
const KIND_RESULT: &str = "result";
const KIND_OLD_ONLY: &str = "old-only";
const KIND_NEW_ONLY: &str = "new-only";
const KIND_ATOMIC_FLAG_GROUP: &str = "atomic-flag-group";
const KIND_UPDATE_PROJECTION: &str = "update-projection";
const KIND_FIELD_PROJECTION: &str = "field-projection";
const KIND_FIELD_REPLACEMENT: &str = "field-replacement";
const KIND_DISCARD: &str = "discard";

const DIR_CANONICAL_TO_PROFILE: &str = "canonical-to-profile";
const DIR_PROFILE_TO_CANONICAL: &str = "profile-to-canonical";
const DIR_BOTH: &str = "both";

fn empty_key() -> SemanticKey {
    SemanticKey {
        category: Category::Type,
        qname: String::new(),
    }
}

fn value_fields(definition: &Definition) -> Vec<&Field> {
    definition
        .fields
        .iter()
        .filter(|field| field.kind == FieldKind::Value)
        .collect()
}

fn find_value_field<'a>(definition: &'a Definition, name: &str) -> Option<&'a Field> {
    definition
        .fields
        .iter()
        .find(|field| field.kind == FieldKind::Value && field.name == name)
}

fn has_value_field(definition: &Definition, name: &str) -> bool {
    find_value_field(definition, name).is_some()
}

fn type_equal(left: &TypeRef, right: &TypeRef) -> bool {
    if left.kind != right.kind
        || left.qname != right.qname
        || left.bare != right.bare
        || left.percent != right.percent
    {
        return false;
    }
    match (&left.arg, &right.arg) {
        (None, None) => true,
        (Some(a), Some(b)) => type_equal(a, b),
        _ => false,
    }
}

#[derive(Debug, Clone)]
struct FieldAlias {
    canonical: String,
    profile: String,
}

fn alias_compatible(canonical: &Field, profile: &Field) -> bool {
    if canonical.ordinal != profile.ordinal || !type_equal(&canonical.ty, &profile.ty) {
        return false;
    }
    match (&canonical.condition, &profile.condition) {
        (None, None) => true,
        (Some(a), Some(b)) => a.presence_only == b.presence_only,
        _ => false,
    }
}

fn find_field_aliases(canonical: &Definition, profile: &Definition) -> Vec<FieldAlias> {
    let canonical_only: Vec<&Field> = value_fields(canonical)
        .into_iter()
        .filter(|field| !has_value_field(profile, &field.name))
        .collect();
    let profile_only: Vec<&Field> = value_fields(profile)
        .into_iter()
        .filter(|field| !has_value_field(canonical, &field.name))
        .collect();

    let mut result = Vec::new();
    let mut used_profile: Vec<String> = Vec::new();
    for canonical_field in &canonical_only {
        let mut matched: Option<&Field> = None;
        for profile_field in &profile_only {
            if used_profile.iter().any(|name| name == &profile_field.name)
                || !alias_compatible(canonical_field, profile_field)
            {
                continue;
            }
            if matched.is_some() {
                matched = None; // Ambiguous candidates must not be guessed.
                break;
            }
            matched = Some(profile_field);
        }
        if let Some(profile_field) = matched {
            used_profile.push(profile_field.name.clone());
            result.push(FieldAlias {
                canonical: canonical_field.name.clone(),
                profile: profile_field.name.clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.canonical
            .cmp(&b.canonical)
            .then_with(|| a.profile.cmp(&b.profile))
    });
    result
}

fn find_field_replacements(
    canonical: &Definition,
    profile: &Definition,
    aliases: &[FieldAlias],
) -> Vec<FieldAlias> {
    let aliased_canonical: Vec<&str> = aliases.iter().map(|a| a.canonical.as_str()).collect();
    let aliased_profile: Vec<&str> = aliases.iter().map(|a| a.profile.as_str()).collect();
    let mut groups: BTreeMap<(String, u8), (Vec<String>, Vec<String>)> = BTreeMap::new();
    for field in value_fields(canonical) {
        if field.condition.is_none()
            || has_value_field(profile, &field.name)
            || aliased_canonical.contains(&field.name.as_str())
        {
            continue;
        }
        let condition = field.condition.as_ref().unwrap();
        groups
            .entry((condition.word.clone(), condition.bit))
            .or_default()
            .0
            .push(field.name.clone());
    }
    for field in value_fields(profile) {
        if field.condition.is_none()
            || has_value_field(canonical, &field.name)
            || aliased_profile.contains(&field.name.as_str())
        {
            continue;
        }
        let condition = field.condition.as_ref().unwrap();
        groups
            .entry((condition.word.clone(), condition.bit))
            .or_default()
            .1
            .push(field.name.clone());
    }
    let mut result = Vec::new();
    for (_, (canonical_names, profile_names)) in groups {
        if canonical_names.len() == 1 && profile_names.len() == 1 {
            result.push(FieldAlias {
                canonical: canonical_names[0].clone(),
                profile: profile_names[0].clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.canonical
            .cmp(&b.canonical)
            .then_with(|| a.profile.cmp(&b.profile))
    });
    result
}

fn alias_map(aliases: &[FieldAlias], canonical_to_profile: bool) -> Vec<(String, String)> {
    aliases
        .iter()
        .map(|alias| {
            if canonical_to_profile {
                (alias.canonical.clone(), alias.profile.clone())
            } else {
                (alias.profile.clone(), alias.canonical.clone())
            }
        })
        .collect()
}

fn lookup_by_value<'a>(map: &'a [(String, String)], value: &str) -> Option<&'a str> {
    map.iter()
        .find(|(_, mapped)| mapped.as_str() == value)
        .map(|(key, _)| key.as_str())
}

fn field_type_string(field: Option<&Field>) -> String {
    field.map(|field| field.ty.as_string()).unwrap_or_default()
}

fn incompatible_fields(source: &Field, target: &Field) -> bool {
    if !type_equal(&source.ty, &target.ty) {
        return true;
    }
    let source_presence = source.condition.as_ref().is_some_and(|c| c.presence_only);
    let target_presence = target.condition.as_ref().is_some_and(|c| c.presence_only);
    source_presence != target_presence
}

fn field_presence(field: Option<&Field>) -> String {
    match field {
        None => "absent".to_owned(),
        Some(field) => match &field.condition {
            None => "required".to_owned(),
            Some(condition) => format!("conditional:{}:{}", condition.word, condition.bit),
        },
    }
}

fn make_field_obligation(
    kind: &'static str,
    layer: i32,
    direction: &'static str,
    source: &Definition,
    target: &Definition,
    source_field: Option<&Field>,
    target_field: Option<&Field>,
) -> Obligation {
    let mut obligation = Obligation {
        key: String::new(),
        kind,
        layer,
        direction,
        semantic: source.key.clone(),
        other_semantic: target.key.clone(),
        wire_id: source.wire_id,
        other_wire_id: target.wire_id,
        field: source_field.map(|f| f.name.clone()).unwrap_or_default(),
        other_field: target_field.map(|f| f.name.clone()).unwrap_or_default(),
        flag_word: String::new(),
        flag_bit: 0,
        fields: Vec::new(),
        source_type: field_type_string(source_field),
        target_type: field_type_string(target_field),
        source_shape: source.signature_shape,
        target_shape: target.signature_shape,
        resolution: None,
    };
    obligation.key = obligation_digest_key(&obligation);
    obligation
}

fn make_definition_obligation(
    kind: &'static str,
    layer: i32,
    direction: &'static str,
    definition: &Definition,
) -> Obligation {
    let mut obligation = Obligation {
        key: String::new(),
        kind,
        layer,
        direction,
        semantic: definition.key.clone(),
        other_semantic: empty_key(),
        wire_id: definition.wire_id,
        other_wire_id: 0,
        field: String::new(),
        other_field: String::new(),
        flag_word: String::new(),
        flag_bit: 0,
        fields: Vec::new(),
        source_type: definition.result.as_string(),
        target_type: String::new(),
        source_shape: definition.signature_shape,
        target_shape: [0; 32],
        resolution: None,
    };
    obligation.key = obligation_digest_key(&obligation);
    obligation
}

fn atomic_flag_group_obligations(
    layer: i32,
    direction: &'static str,
    source: &Definition,
    target: &Definition,
    aliases: &[(String, String)],
) -> Vec<Obligation> {
    let mut groups: BTreeMap<(String, u8), Vec<&Field>> = BTreeMap::new();
    for field in value_fields(target) {
        if let Some(condition) = &field.condition {
            groups
                .entry((condition.word.clone(), condition.bit))
                .or_default()
                .push(field);
        }
    }
    let mut result = Vec::new();
    for ((word, bit), fields) in groups {
        if fields.len() <= 1 {
            continue;
        }
        let mut expected: Option<String> = None;
        let mut compatible = true;
        let mut names = Vec::new();
        for (index, field) in fields.iter().enumerate() {
            names.push(field.name.clone());
            let source_name = lookup_by_value(aliases, &field.name)
                .map(str::to_owned)
                .unwrap_or_else(|| field.name.clone());
            let presence = field_presence(find_value_field(source, &source_name));
            if index == 0 {
                expected = Some(presence);
            } else if Some(&presence) != expected.as_ref() {
                compatible = false;
            }
        }
        if compatible {
            continue;
        }
        names.sort();
        let mut obligation = Obligation {
            key: String::new(),
            kind: KIND_ATOMIC_FLAG_GROUP,
            layer,
            direction,
            semantic: source.key.clone(),
            other_semantic: target.key.clone(),
            wire_id: source.wire_id,
            other_wire_id: target.wire_id,
            field: String::new(),
            other_field: String::new(),
            flag_word: word,
            flag_bit: bit,
            fields: names,
            source_type: String::new(),
            target_type: String::new(),
            source_shape: source.signature_shape,
            target_shape: target.signature_shape,
            resolution: None,
        };
        obligation.key = obligation_digest_key(&obligation);
        result.push(obligation);
    }
    result
}

fn compare_direction(
    layer: i32,
    source: &Definition,
    target: &Definition,
    direction: &'static str,
    aliases: &[(String, String)],
    replacements: &[(String, String)],
) -> Vec<Obligation> {
    let mut result = Vec::new();
    for target_field in &target.fields {
        if target_field.kind != FieldKind::Value {
            continue;
        }
        let mut source_name = target_field.name.clone();
        if let Some(candidate) = lookup_by_value(aliases, &target_field.name) {
            source_name = candidate.to_owned();
        }
        let mut paired = false;
        if let Some(candidate) = lookup_by_value(replacements, &target_field.name) {
            source_name = candidate.to_owned();
            paired = true;
        }
        let source_field = find_value_field(source, &source_name);
        let Some(source_field) = source_field else {
            if target_field.condition.is_none() {
                result.push(make_field_obligation(
                    KIND_REQUIRED,
                    layer,
                    direction,
                    source,
                    target,
                    None,
                    Some(target_field),
                ));
            }
            continue;
        };
        if paired {
            continue;
        }
        if incompatible_fields(source_field, target_field) {
            result.push(make_field_obligation(
                KIND_INCOMPATIBLE,
                layer,
                direction,
                source,
                target,
                Some(source_field),
                Some(target_field),
            ));
            continue;
        }
        if source_field.condition.is_some() && target_field.condition.is_none() {
            result.push(make_field_obligation(
                KIND_REQUIRED,
                layer,
                direction,
                source,
                target,
                Some(source_field),
                Some(target_field),
            ));
        }
    }
    let mut combined = aliases.to_vec();
    combined.extend(replacements.iter().cloned());
    result.extend(atomic_flag_group_obligations(
        layer, direction, source, target, &combined,
    ));
    result
}

fn projected_canonical_fields(
    layer: i32,
    canonical: &Definition,
    profile: &Definition,
    aliases: &[(String, String)],
    replacements: &[(String, String)],
) -> Vec<Obligation> {
    let mut result = Vec::new();
    for field in value_fields(canonical) {
        let mut profile_name = field.name.clone();
        if let Some(mapped) = aliases
            .iter()
            .find(|(canonical, _)| canonical == &field.name)
            .map(|(_, profile)| profile.clone())
        {
            profile_name = mapped;
        }
        if let Some(mapped) = replacements
            .iter()
            .find(|(canonical, _)| canonical == &field.name)
            .map(|(_, profile)| profile.clone())
        {
            profile_name = mapped;
        }
        if has_value_field(profile, &profile_name) {
            continue;
        }
        let mut obligation = make_field_obligation(
            KIND_FIELD_PROJECTION,
            layer,
            DIR_CANONICAL_TO_PROFILE,
            canonical,
            profile,
            Some(field),
            None,
        );
        obligation.resolution = Some(Resolution {
            action: "drop".to_owned(),
            ..Resolution::default()
        });
        result.push(obligation);
    }
    result
}

fn discarded_profile_fields(
    layer: i32,
    profile: &Definition,
    canonical: &Definition,
    aliases: &[(String, String)],
    replacements: &[(String, String)],
) -> Vec<Obligation> {
    let mut result = Vec::new();
    for field in value_fields(profile) {
        let mut canonical_name = field.name.clone();
        if let Some(mapped) = aliases
            .iter()
            .find(|(profile, _)| profile == &field.name)
            .map(|(_, canonical)| canonical.clone())
        {
            canonical_name = mapped;
        }
        if let Some(mapped) = replacements
            .iter()
            .find(|(profile, _)| profile == &field.name)
            .map(|(_, canonical)| canonical.clone())
        {
            canonical_name = mapped;
        }
        if has_value_field(canonical, &canonical_name) {
            continue;
        }
        result.push(make_field_obligation(
            KIND_DISCARD,
            layer,
            DIR_PROFILE_TO_CANONICAL,
            profile,
            canonical,
            Some(field),
            None,
        ));
    }
    result
}

fn compare_layer_definitions(
    layer: i32,
    canonical: &Definition,
    profile: &Definition,
) -> Vec<Obligation> {
    let aliases = find_field_aliases(canonical, profile);
    let replacements = find_field_replacements(canonical, profile, &aliases);
    let mut result = Vec::new();
    for alias in &aliases {
        let mut obligation = Obligation {
            key: String::new(),
            kind: KIND_ALIAS,
            layer,
            direction: DIR_BOTH,
            semantic: canonical.key.clone(),
            other_semantic: empty_key(),
            wire_id: canonical.wire_id,
            other_wire_id: profile.wire_id,
            field: alias.canonical.clone(),
            other_field: alias.profile.clone(),
            flag_word: String::new(),
            flag_bit: 0,
            fields: Vec::new(),
            source_type: field_type_string(find_value_field(canonical, &alias.canonical)),
            target_type: field_type_string(find_value_field(profile, &alias.profile)),
            source_shape: canonical.signature_shape,
            target_shape: profile.signature_shape,
            resolution: None,
        };
        obligation.key = obligation_digest_key(&obligation);
        result.push(obligation);
    }
    for replacement in &replacements {
        let mut obligation = Obligation {
            key: String::new(),
            kind: KIND_FIELD_REPLACEMENT,
            layer,
            direction: DIR_BOTH,
            semantic: canonical.key.clone(),
            other_semantic: profile.key.clone(),
            wire_id: canonical.wire_id,
            other_wire_id: profile.wire_id,
            field: replacement.canonical.clone(),
            other_field: replacement.profile.clone(),
            flag_word: String::new(),
            flag_bit: 0,
            fields: Vec::new(),
            source_type: field_type_string(find_value_field(canonical, &replacement.canonical)),
            target_type: field_type_string(find_value_field(profile, &replacement.profile)),
            source_shape: canonical.signature_shape,
            target_shape: profile.signature_shape,
            resolution: None,
        };
        obligation.key = obligation_digest_key(&obligation);
        result.push(obligation);
    }
    result.extend(compare_direction(
        layer,
        canonical,
        profile,
        DIR_CANONICAL_TO_PROFILE,
        &alias_map(&aliases, true),
        &alias_map(&replacements, true),
    ));
    result.extend(compare_direction(
        layer,
        profile,
        canonical,
        DIR_PROFILE_TO_CANONICAL,
        &alias_map(&aliases, false),
        &alias_map(&replacements, false),
    ));
    result.extend(projected_canonical_fields(
        layer,
        canonical,
        profile,
        &alias_map(&aliases, true),
        &alias_map(&replacements, true),
    ));
    result.extend(discarded_profile_fields(
        layer,
        profile,
        canonical,
        &alias_map(&aliases, false),
        &alias_map(&replacements, false),
    ));
    if canonical.key.category == Category::Function
        && !type_equal(&canonical.result, &profile.result)
    {
        let mut to_profile = make_result_obligation(
            layer,
            DIR_CANONICAL_TO_PROFILE,
            canonical,
            profile,
            canonical.wire_id,
            profile.wire_id,
        );
        to_profile.key = obligation_digest_key(&to_profile);
        result.push(to_profile);
        let mut to_canonical = make_result_obligation(
            layer,
            DIR_PROFILE_TO_CANONICAL,
            canonical,
            profile,
            profile.wire_id,
            canonical.wire_id,
        );
        to_canonical.key = obligation_digest_key(&to_canonical);
        result.push(to_canonical);
    }
    result
}

fn make_result_obligation(
    layer: i32,
    direction: &'static str,
    canonical: &Definition,
    profile: &Definition,
    wire_id: u32,
    other_wire_id: u32,
) -> Obligation {
    let (source, target) = if direction == DIR_CANONICAL_TO_PROFILE {
        (canonical, profile)
    } else {
        (profile, canonical)
    };
    Obligation {
        key: String::new(),
        kind: KIND_RESULT,
        layer,
        direction,
        semantic: canonical.key.clone(),
        other_semantic: empty_key(),
        wire_id,
        other_wire_id,
        field: String::new(),
        other_field: String::new(),
        flag_word: String::new(),
        flag_bit: 0,
        fields: Vec::new(),
        source_type: source.result.as_string(),
        target_type: target.result.as_string(),
        source_shape: source.signature_shape,
        target_shape: target.signature_shape,
        resolution: None,
    }
}

fn build_field_mappings(canonical: &Definition, profile: &Definition) -> Vec<FieldMapping> {
    let aliases = find_field_aliases(canonical, profile);
    let replacements = find_field_replacements(canonical, profile, &aliases);
    let mut profile_to_canonical = alias_map(&aliases, false);
    profile_to_canonical.extend(alias_map(&replacements, false));
    let mut result = Vec::new();
    for (profile_ordinal, field) in profile.fields.iter().enumerate() {
        let mut canonical_name = field.name.clone();
        if let Some(mapped) = profile_to_canonical
            .iter()
            .find(|(profile, _)| profile == &field.name)
            .map(|(_, canonical)| canonical.clone())
        {
            canonical_name = mapped;
        }
        let canonical_ordinal = canonical
            .fields
            .iter()
            .position(|candidate| candidate.name == canonical_name);
        if canonical_ordinal.is_none() {
            canonical_name.clear();
        }
        result.push(FieldMapping {
            canonical_ordinal,
            profile_ordinal,
            canonical_name,
            profile_name: field.name.clone(),
        });
    }
    result
}

fn is_update_definition(definition: &Definition) -> bool {
    definition.key.category == Category::Type
        && definition.result.kind == TypeKind::Named
        && !definition.result.bare
        && definition.result.qname == "Update"
}

/// Analyzes conversions for every loaded layer and applies the reviewed policy.
///
/// # Errors
///
/// Returns a message on duplicate obligation keys, a stale policy entry or an
/// action invalid for its obligation kind.
pub(crate) fn analyze(universe: &Universe, policy: &Policy) -> Result<ConversionPlan, String> {
    let canonical_layer = universe.canonical_layer;
    let mut plan = ConversionPlan {
        canonical_layer,
        profiles: BTreeMap::new(),
        obligations: Vec::new(),
    };
    let keys: Vec<SemanticKey> = universe.families.keys().cloned().collect();
    for layer in universe.layers() {
        let mut profile = ProfileConversion {
            layer,
            families: BTreeMap::new(),
        };
        for key in &keys {
            let family = &universe.families[key];
            let canonical = family.by_layer.get(&canonical_layer);
            let target = family.by_layer.get(&layer);
            let mut conversion = FamilyConversion {
                key: key.clone(),
                canonical: canonical.cloned(),
                profile: target.cloned(),
                availability: Availability::Absent,
                body_changed: false,
                result_changed: false,
                fields: Vec::new(),
                obligations: Vec::new(),
            };
            match (canonical, target) {
                (None, Some(target)) => {
                    conversion.availability = Availability::ProfileOnly;
                    conversion.body_changed = true;
                    plan.obligations.push(make_definition_obligation(
                        KIND_OLD_ONLY,
                        layer,
                        DIR_PROFILE_TO_CANONICAL,
                        target,
                    ));
                }
                (Some(canonical), None) => {
                    conversion.availability = Availability::CanonicalOnly;
                    conversion.body_changed = true;
                    let mut unavailable = make_definition_obligation(
                        KIND_NEW_ONLY,
                        layer,
                        DIR_CANONICAL_TO_PROFILE,
                        canonical,
                    );
                    unavailable.resolution = Some(Resolution {
                        action: "drop".to_owned(),
                        ..Resolution::default()
                    });
                    plan.obligations.push(unavailable);
                    if is_update_definition(canonical) {
                        plan.obligations.push(make_definition_obligation(
                            KIND_UPDATE_PROJECTION,
                            layer,
                            DIR_CANONICAL_TO_PROFILE,
                            canonical,
                        ));
                    }
                }
                (Some(canonical), Some(target)) => {
                    conversion.availability = Availability::Present;
                    conversion.body_changed = canonical.wire_id != target.wire_id
                        || canonical.body_shape != target.body_shape;
                    conversion.result_changed = key.category == Category::Function
                        && !type_equal(&canonical.result, &target.result);
                    conversion.fields = build_field_mappings(canonical, target);
                    plan.obligations
                        .extend(compare_layer_definitions(layer, canonical, target));
                }
                (None, None) => {
                    conversion.availability = Availability::Absent;
                }
            }
            profile.families.insert(key.clone(), conversion);
        }
        plan.profiles.insert(layer, profile);
    }

    plan.obligations.sort_by(|a, b| a.key.cmp(&b.key));
    for window in plan.obligations.windows(2) {
        if window[0].key == window[1].key {
            return Err(format!(
                "E_DUPLICATE_LAYER_OBLIGATION: generated key {:?} is repeated",
                window[1].key
            ));
        }
    }
    apply_policy(&mut plan.obligations, policy)?;
    // Attach obligations to their families.
    for obligation in &plan.obligations {
        let Some(profile) = plan.profiles.get_mut(&obligation.layer) else {
            return Err(format!(
                "obligation {} references missing layer {}",
                obligation.key, obligation.layer
            ));
        };
        if obligation.semantic.qname.is_empty() {
            continue;
        }
        let Some(family) = profile.families.get_mut(&obligation.semantic) else {
            return Err(format!(
                "obligation {} references missing family {}",
                obligation.key,
                obligation.semantic.as_string()
            ));
        };
        family.obligations.push(obligation.clone());
    }
    Ok(plan)
}

fn apply_policy(obligations: &mut [Obligation], policy: &Policy) -> Result<(), String> {
    let mut seen: Vec<String> = Vec::new();
    for entry in &policy.entries {
        if seen.iter().any(|key| key == &entry.key) {
            return Err(format!(
                "E_DUPLICATE_LAYER_POLICY: key {:?} is repeated",
                entry.key
            ));
        }
        seen.push(entry.key.clone());
        let position = obligations.iter().position(|item| item.key == entry.key);
        let Some(position) = position else {
            return Err(format!(
                "E_STALE_LAYER_POLICY: key {:?} does not match a generated obligation",
                entry.key
            ));
        };
        validate_resolution(obligations[position].kind, &entry.resolution)
            .map_err(|error| format!("E_INVALID_LAYER_POLICY: key {:?}: {error}", entry.key))?;
        if obligations[position].kind == KIND_OLD_ONLY
            && matches!(entry.resolution.action.as_str(), "alias" | "adapter")
        {
            let target = parse_semantic_target(entry.resolution.target.trim())?;
            if target.category != obligations[position].semantic.category {
                return Err(format!(
                    "E_INVALID_LAYER_POLICY: key {:?}: old-only {} target has another category",
                    entry.key,
                    obligations[position].semantic.as_string()
                ));
            }
        }
        obligations[position].resolution = Some(entry.resolution.clone());
    }
    Ok(())
}

fn parse_semantic_target(value: &str) -> Result<SemanticKey, String> {
    let Some((category, qname)) = value.split_once(':') else {
        return Err(format!("invalid semantic target {value:?}"));
    };
    if qname.is_empty() || qname.trim() != qname {
        return Err(format!("invalid semantic target {value:?}"));
    }
    let category = match category {
        "function" => Category::Function,
        "type" => Category::Type,
        _ => return Err(format!("invalid semantic target category {category:?}")),
    };
    Ok(SemanticKey {
        category,
        qname: qname.to_owned(),
    })
}

const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

fn validate_resolution(kind: &str, resolution: &Resolution) -> Result<(), String> {
    let action = resolution.action.as_str();
    let allowed = match kind {
        KIND_ALIAS => matches!(action, "alias" | "adapter" | "reject"),
        KIND_REQUIRED => matches!(action, "default" | "adapter" | "reject"),
        KIND_INCOMPATIBLE | KIND_ATOMIC_FLAG_GROUP | KIND_RESULT => {
            matches!(action, "adapter" | "reject")
        }
        KIND_OLD_ONLY => matches!(action, "alias" | "adapter" | "reject"),
        KIND_NEW_ONLY => matches!(action, "unavailable" | "drop" | "project" | "reject"),
        KIND_UPDATE_PROJECTION => matches!(action, "drop" | "project" | "reject"),
        KIND_FIELD_PROJECTION => {
            matches!(action, "drop" | "adapter" | "project" | "reject-if-present")
        }
        KIND_FIELD_REPLACEMENT => matches!(action, "adapter" | "reject"),
        KIND_DISCARD => matches!(action, "drop" | "adapter" | "reject"),
        _ => false,
    };
    if !allowed {
        return Err(format!("action {action:?} is not valid for {kind}"));
    }
    let hook = resolution.hook.trim();
    let requires_hook = matches!(action, "adapter" | "project" | "alias");
    if requires_hook {
        if !crate::parse::is_valid_identifier(hook) || GO_KEYWORDS.contains(&hook) {
            return Err(format!(
                "action {action:?} requires a non-keyword hook identifier"
            ));
        }
    } else if !hook.is_empty() {
        return Err(format!("action {action:?} does not accept hook {hook:?}"));
    }
    let target = resolution.target.trim();
    if kind == KIND_OLD_ONLY && matches!(action, "alias" | "adapter") {
        parse_semantic_target(target)?;
    } else if !target.is_empty() {
        return Err(format!(
            "action {action:?} for {kind} does not accept target {target:?}"
        ));
    }
    Ok(())
}

fn obligation_digest_key(obligation: &Obligation) -> String {
    let mut writer = DigestWriter::new("gotd.tl.layer-obligation.v1");
    writer.string(obligation.kind);
    writer.uint64(obligation.layer as u64);
    writer.string(obligation.direction);
    writer.string(&obligation.semantic.as_string());
    writer.string(&obligation.other_semantic.as_string());
    writer.uint32(obligation.wire_id);
    writer.uint32(obligation.other_wire_id);
    writer.string(&obligation.field);
    writer.string(&obligation.other_field);
    writer.string(&obligation.flag_word);
    writer.uint32(u32::from(obligation.flag_bit));
    writer.uint64(obligation.fields.len() as u64);
    for field in &obligation.fields {
        writer.string(field);
    }
    writer.string(&obligation.source_type);
    writer.string(&obligation.target_type);
    writer.string(&to_hex(&obligation.source_shape));
    writer.string(&to_hex(&obligation.target_shape));
    format!(
        "layer-obligation/v1/{}/{}",
        obligation.kind,
        to_hex(&writer.finish())
    )
}

#[cfg(test)]
mod tests {
    use super::{Availability, analyze};
    use crate::model::{SemanticKey, SourceRef, build_schema};
    use crate::parse::{Category, parse_schema};
    use crate::schema::{Policy, PolicyEntry, Resolution};
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
    fn canonical_equal_needs_no_policy() {
        let universe = universe(&[(231, "foo#1 a:int = Foo;\n"), (232, "foo#1 a:int = Foo;\n")]);
        let plan = analyze(
            &universe,
            &Policy {
                entries: Vec::new(),
            },
        )
        .unwrap();
        let profile = plan.profile(232).unwrap();
        let key = SemanticKey {
            category: Category::Type,
            qname: "foo".to_owned(),
        };
        assert_eq!(
            profile.family(&key).unwrap().availability,
            Availability::Present
        );
        assert!(plan.unresolved().is_empty());
    }

    #[test]
    fn unreviewed_difference_is_unresolved() {
        // Renamed field with the same ordinal and type: an alias obligation.
        let universe = universe(&[(231, "foo#1 a:int = Foo;\n"), (232, "foo#1 b:int = Foo;\n")]);
        let plan = analyze(
            &universe,
            &Policy {
                entries: Vec::new(),
            },
        )
        .unwrap();
        let unresolved = plan.unresolved();
        assert!(
            unresolved.iter().any(|o| o.kind == "alias"),
            "expected an unresolved alias obligation"
        );
    }

    #[test]
    fn stale_policy_entry_fails() {
        let universe = universe(&[(232, "foo#1 a:int = Foo;\n")]);
        let policy = Policy {
            entries: vec![PolicyEntry {
                key: "layer-obligation/v1/discard/deadbeef".to_owned(),
                resolution: Resolution {
                    action: "drop".to_owned(),
                    ..Resolution::default()
                },
            }],
        };
        let error = analyze(&universe, &policy).unwrap_err();
        assert!(error.contains("E_STALE_LAYER_POLICY"), "{error}");
    }

    #[test]
    fn reviewed_difference_resolves() {
        let universe = universe(&[(231, "foo#1 a:int = Foo;\n"), (232, "foo#1 b:int = Foo;\n")]);
        // Discover the generated alias key, then resolve it exactly as a review
        // would: a stale key must remain a hard error.
        let plan = analyze(
            &universe,
            &Policy {
                entries: Vec::new(),
            },
        )
        .unwrap();
        let alias = plan
            .unresolved()
            .into_iter()
            .find(|o| o.kind == "alias")
            .expect("alias obligation")
            .key
            .clone();
        let policy = Policy {
            entries: vec![PolicyEntry {
                key: alias.clone(),
                resolution: Resolution {
                    action: "alias".to_owned(),
                    hook: "layerRenameFoo".to_owned(),
                    ..Resolution::default()
                },
            }],
        };
        let resolved = analyze(&universe, &policy).unwrap();
        assert!(resolved.unresolved().is_empty());
        assert!(resolved.obligations.iter().any(|o| o.key == alias));
    }
}
