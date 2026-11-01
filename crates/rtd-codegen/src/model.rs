//! The semantic model: category-qualified definition families, type references
//! and the domain-separated shape digests that make cross-layer comparison
//! stable.
//!
//! This is a faithful port of `td/gen/semantic` (`model.go`, `type_ref.go`,
//! `shape.go`). In particular the digest domains and byte layout must match so
//! that semantic identities and policy keys are identical to the Go reference.

use sha2::{Digest, Sha256};

use crate::parse::{Category, RawDefinition, RawSchema, RawType};

/// The category-qualified identity of a constructor or method. Wire identifiers
/// are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SemanticKey {
    /// Constructor or method.
    pub(crate) category: Category,
    /// Dotted qualified name.
    pub(crate) qname: String,
}

impl SemanticKey {
    /// Renders the key as `type:foo` or `function:a.b`.
    pub(crate) fn as_string(&self) -> String {
        format!("{}:{}", self.category.as_str(), self.qname)
    }
}

/// A 32-byte SHA-256 shape digest.
pub(crate) type Shape = [u8; 32];

/// A digest of the semantic identity, independent of wire identity.
pub(crate) fn semantic_stable_id(key: &SemanticKey) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(b"gotd.tl.layer-semantic.v1\x00");
    hasher.update(key.as_string().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

/// The wire-relevant kind of a type reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeKind {
    /// A TL primitive or built-in value.
    Primitive = 0,
    /// A constructor class or an exact bare constructor.
    Named = 1,
    /// A boxed `Vector<T>` or bare `vector<T>`.
    Vector = 2,
    /// A generic parameter such as `!X`.
    GenericRef = 3,
}

/// A recursive, lossless type reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeRef {
    /// The wire-relevant kind.
    pub(crate) kind: TypeKind,
    /// The dotted qualified name.
    pub(crate) qname: String,
    /// Whether the reference is bare.
    pub(crate) bare: bool,
    /// Whether the reference used the `%` marker.
    pub(crate) percent: bool,
    /// The element type for vectors and generic applications.
    pub(crate) arg: Option<Box<TypeRef>>,
}

const PRIMITIVES: &[&str] = &[
    "int", "int32", "int53", "int64", "long", "int128", "int256", "double", "string", "bytes",
    "bool", "true", "false", "Int", "Long", "Double", "String", "Bytes", "Bool", "True", "Object",
];

impl TypeRef {
    /// Builds a reference from a parsed type, binding generic parameters.
    fn from_raw(raw: &RawType, generics: &[String]) -> TypeRef {
        let qname = raw.qualified();
        let mut kind = if raw.generic_ref {
            TypeKind::GenericRef
        } else if raw.name == "Vector" || raw.name == "vector" {
            TypeKind::Vector
        } else if PRIMITIVES.contains(&qname.as_str()) {
            TypeKind::Primitive
        } else {
            TypeKind::Named
        };
        let arg = raw
            .arg
            .as_ref()
            .map(|inner| Box::new(TypeRef::from_raw(inner, generics)));
        if kind == TypeKind::Named && arg.is_none() && generics.iter().any(|p| p == &qname) {
            kind = TypeKind::GenericRef;
        }
        TypeRef {
            kind,
            qname,
            bare: raw.bare,
            percent: raw.percent,
            arg,
        }
    }

    /// Normalizes TL spellings that share one payload codec.
    fn wire_name(&self) -> &str {
        if self.kind != TypeKind::Primitive {
            return &self.qname;
        }
        match self.qname.as_str() {
            "int" | "int32" | "Int" => "int32",
            "int53" | "long" | "int64" | "Long" => "int64",
            "double" | "Double" => "float64",
            "string" | "String" | "bytes" | "Bytes" => "tl-bytes",
            "Bool" | "bool" | "true" | "false" | "True" => "tl-bool",
            other => other,
        }
    }

    /// Renders a compact TL spelling for diagnostics and policy notes.
    pub(crate) fn as_string(&self) -> String {
        let mut out = String::new();
        if self.kind == TypeKind::GenericRef {
            out.push('!');
        } else if self.percent {
            out.push('%');
        }
        out.push_str(&self.qname);
        if let Some(arg) = &self.arg {
            out.push('<');
            out.push_str(&arg.as_string());
            out.push('>');
        }
        out
    }
}

/// Distinguishes a flags word from an encoded field value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldKind {
    /// A value field.
    Value = 0,
    /// An inline flags word (`#`).
    FlagsWord = 1,
}

/// A presence condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Condition {
    /// Flags word name.
    pub(crate) word: String,
    /// Bit index.
    pub(crate) bit: u8,
    /// Whether the field is presence-only (`flags.N?true`).
    pub(crate) presence_only: bool,
}

/// One ordered TL parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    /// Value or flags word.
    pub(crate) kind: FieldKind,
    /// Parameter name.
    pub(crate) name: String,
    /// Zero-based position among parameters.
    pub(crate) ordinal: usize,
    /// The value type (default for flags words).
    pub(crate) ty: TypeRef,
    /// Presence condition, if any.
    pub(crate) condition: Option<Condition>,
}

fn empty_type() -> TypeRef {
    TypeRef {
        kind: TypeKind::Primitive,
        qname: String::new(),
        bare: false,
        percent: false,
        arg: None,
    }
}

/// One constructor or method in one concrete layer.
#[derive(Debug, Clone)]
pub(crate) struct Definition {
    /// Semantic key.
    pub(crate) key: SemanticKey,
    /// Explicit wire identifier.
    pub(crate) wire_id: u32,
    /// Declared generic parameters.
    pub(crate) generic_params: Vec<String>,
    /// Ordered fields.
    pub(crate) fields: Vec<Field>,
    /// Result type.
    pub(crate) result: TypeRef,
    /// Whether the definition is bare.
    pub(crate) base: bool,
    /// Payload shape digest.
    pub(crate) body_shape: Shape,
    /// Body + result shape digest.
    pub(crate) signature_shape: Shape,
    /// Encoded-payload-only shape digest.
    pub(crate) wire_shape: Shape,
}

/// The provenance of one layer schema.
#[derive(Debug, Clone)]
pub(crate) struct SourceRef {
    /// Layer number.
    pub(crate) layer: i32,
    /// Upstream repository.
    pub(crate) repository: String,
    /// Upstream commit.
    pub(crate) commit: String,
    /// Upstream blob (SHA-1 object id).
    pub(crate) blob: String,
    /// Upstream source path.
    pub(crate) path: String,
    /// Local file name.
    pub(crate) file: String,
    /// SHA-256 of the schema bytes.
    pub(crate) sha256: Shape,
}

/// A validated semantic view of one TL layer.
#[derive(Debug, Clone)]
pub(crate) struct SchemaModel {
    /// Layer number.
    pub(crate) layer: i32,
    /// Provenance.
    pub(crate) source: SourceRef,
    /// Definitions in source order.
    pub(crate) definitions: Vec<Definition>,
    /// Lookup by semantic key.
    pub(crate) by_key: std::collections::BTreeMap<SemanticKey, usize>,
    /// Lookup by wire identifier.
    pub(crate) by_wire: std::collections::BTreeMap<u32, usize>,
    /// Constructor keys grouped by result class.
    pub(crate) constructors_by_class: std::collections::BTreeMap<String, Vec<usize>>,
}

impl SchemaModel {
    /// Finds a definition by semantic key.
    pub(crate) fn find_key(&self, key: &SemanticKey) -> Option<&Definition> {
        self.by_key.get(key).map(|index| &self.definitions[*index])
    }

    /// Finds a definition by wire identifier.
    pub(crate) fn find_wire(&self, id: u32) -> Option<&Definition> {
        self.by_wire.get(&id).map(|index| &self.definitions[*index])
    }

    /// Returns the constructors of a result class.
    pub(crate) fn class_constructors(&self, class: &str) -> Vec<&Definition> {
        self.constructors_by_class
            .get(class)
            .map(|indices| indices.iter().map(|i| &self.definitions[*i]).collect())
            .unwrap_or_default()
    }
}

/// An error raised while building a semantic model.
#[derive(Debug)]
pub(crate) struct ModelError(pub(crate) String);

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Converts a parsed schema into a validated semantic model.
///
/// # Errors
///
/// Returns an error for duplicate keys or identifiers, invalid flag references
/// or unknown type references.
pub(crate) fn build_schema(
    schema: &RawSchema,
    source: SourceRef,
) -> Result<SchemaModel, ModelError> {
    build_schema_inner(schema, source, true)
}

/// Builds a model without class-reference validation.
///
/// This is used for the single non-layered MTProto service schema, which
/// contains an RPC whose result class has no constructor (`http_wait`).
///
/// # Errors
///
/// Returns an error for duplicate keys or identifiers and invalid flags.
pub(crate) fn build_schema_relaxed(
    schema: &RawSchema,
    source: SourceRef,
) -> Result<SchemaModel, ModelError> {
    build_schema_inner(schema, source, false)
}

fn build_schema_inner(
    schema: &RawSchema,
    source: SourceRef,
    validate: bool,
) -> Result<SchemaModel, ModelError> {
    let layer = source.layer;
    if schema.layer != 0 && schema.layer != layer {
        return Err(ModelError(format!(
            "layer {layer} file declares layer {}",
            schema.layer
        )));
    }
    let mut model = SchemaModel {
        layer,
        source,
        definitions: Vec::with_capacity(schema.definitions.len()),
        by_key: std::collections::BTreeMap::new(),
        by_wire: std::collections::BTreeMap::new(),
        constructors_by_class: std::collections::BTreeMap::new(),
    };

    for raw in &schema.definitions {
        let definition = build_definition(raw, layer)?;
        if model.by_key.contains_key(&definition.key) {
            return Err(ModelError(format!(
                "layer {layer} duplicate definition {}",
                definition.key.as_string()
            )));
        }
        if model.by_wire.contains_key(&definition.wire_id) {
            return Err(ModelError(format!(
                "layer {layer} duplicate wire id {:#010x} for {}",
                definition.wire_id,
                definition.key.as_string()
            )));
        }
        let index = model.definitions.len();
        model.by_key.insert(definition.key.clone(), index);
        model.by_wire.insert(definition.wire_id, index);
        if definition.key.category == Category::Type {
            model
                .constructors_by_class
                .entry(definition.result.qname.clone())
                .or_default()
                .push(index);
        }
        model.definitions.push(definition);
    }
    if validate {
        validate_references(&model).map_err(ModelError)?;
    }
    Ok(model)
}

fn build_definition(raw: &RawDefinition, layer: i32) -> Result<Definition, ModelError> {
    let key = SemanticKey {
        category: raw.category,
        qname: raw.qualified(),
    };
    let result = TypeRef::from_raw(&raw.result, &raw.generic_params);
    let mut fields = Vec::with_capacity(raw.params.len());
    for (ordinal, param) in raw.params.iter().enumerate() {
        let mut field = Field {
            kind: if param.flags {
                FieldKind::FlagsWord
            } else {
                FieldKind::Value
            },
            name: param.name.clone(),
            ordinal,
            ty: empty_type(),
            condition: None,
        };
        if !param.flags {
            field.ty = TypeRef::from_raw(&param.ty, &raw.generic_params);
        }
        if let Some(flag) = &param.flag {
            if flag.bit > 31 {
                return Err(ModelError(format!(
                    "layer {layer} {} field {:?} has invalid flag bit {}",
                    key.as_string(),
                    param.name,
                    flag.bit
                )));
            }
            field.condition = Some(Condition {
                word: flag.word.clone(),
                bit: flag.bit,
                presence_only: param.ty.name == "true" && param.ty.namespace.is_empty(),
            });
        }
        fields.push(field);
    }
    let mut definition = Definition {
        key,
        wire_id: raw.id,
        generic_params: raw.generic_params.clone(),
        fields,
        result,
        base: raw.base,
        body_shape: [0; 32],
        signature_shape: [0; 32],
        wire_shape: [0; 32],
    };
    validate_definition(&definition, layer)?;
    definition.body_shape = body_shape(&definition);
    definition.signature_shape = signature_shape(&definition);
    definition.wire_shape = payload_wire_shape(&definition);
    Ok(definition)
}

fn validate_definition(def: &Definition, layer: i32) -> Result<(), ModelError> {
    let mut fields = std::collections::HashSet::new();
    let mut flag_words = std::collections::HashMap::new();
    for field in &def.fields {
        if field.name.is_empty() {
            return Err(ModelError(format!(
                "layer {layer} {} field at ordinal {} has no name",
                def.key.as_string(),
                field.ordinal
            )));
        }
        if !fields.insert(field.name.clone()) {
            return Err(ModelError(format!(
                "layer {layer} {} duplicate field {:?}",
                def.key.as_string(),
                field.name
            )));
        }
        if field.kind == FieldKind::FlagsWord {
            flag_words.insert(field.name.clone(), field.ordinal);
        }
    }
    for field in &def.fields {
        if let Some(condition) = &field.condition {
            let flags_ordinal = flag_words.get(&condition.word).copied().ok_or_else(|| {
                ModelError(format!(
                    "layer {layer} {} field {:?} refers to missing flags word {:?}",
                    def.key.as_string(),
                    field.name,
                    condition.word
                ))
            })?;
            if flags_ordinal >= field.ordinal {
                return Err(ModelError(format!(
                    "layer {layer} {} field {:?} refers to flags word {:?} that is not before it",
                    def.key.as_string(),
                    field.name,
                    condition.word
                )));
            }
        }
        if field.kind == FieldKind::Value {
            validate_generic_refs(&field.ty, &def.generic_params, layer, &def.key)?;
        }
    }
    validate_generic_refs(&def.result, &def.generic_params, layer, &def.key)?;
    Ok(())
}

fn validate_generic_refs(
    reference: &TypeRef,
    params: &[String],
    layer: i32,
    key: &SemanticKey,
) -> Result<(), ModelError> {
    if reference.kind == TypeKind::GenericRef && !params.iter().any(|p| p == &reference.qname) {
        return Err(ModelError(format!(
            "layer {layer} {} undefined generic parameter {:?}",
            key.as_string(),
            reference.qname
        )));
    }
    if let Some(arg) = &reference.arg {
        validate_generic_refs(arg, params, layer, key)?;
    }
    Ok(())
}

fn validate_references(model: &SchemaModel) -> Result<(), String> {
    let constructors: Vec<&str> = model
        .definitions
        .iter()
        .filter(|def| def.key.category == Category::Type)
        .map(|def| def.key.qname.as_str())
        .collect();
    for def in &model.definitions {
        for field in &def.fields {
            if field.kind == FieldKind::FlagsWord {
                continue;
            }
            validate_reference(model, &field.ty, &constructors).map_err(|error| {
                format!(
                    "layer {} {} field {:?}: {error}",
                    model.layer,
                    def.key.as_string(),
                    field.name
                )
            })?;
        }
        validate_reference(model, &def.result, &constructors).map_err(|error| {
            format!(
                "layer {} {} result: {error}",
                model.layer,
                def.key.as_string()
            )
        })?;
    }
    Ok(())
}

fn validate_reference(
    model: &SchemaModel,
    reference: &TypeRef,
    constructors: &[&str],
) -> Result<(), String> {
    if let Some(arg) = &reference.arg {
        validate_reference(model, arg, constructors)?;
    }
    if reference.kind != TypeKind::Named {
        return Ok(());
    }
    if reference.bare || reference.percent {
        if !constructors.contains(&reference.qname.as_str()) {
            return Err(format!("unknown bare constructor {:?}", reference.qname));
        }
        return Ok(());
    }
    if model
        .constructors_by_class
        .iter()
        .all(|(c, _)| c != &reference.qname)
    {
        return Err(format!("unknown constructor class {:?}", reference.qname));
    }
    Ok(())
}

/// A domain-separated SHA-256 writer matching `semantic.shapeWriter`.
struct Hasher {
    inner: Sha256,
}

impl Hasher {
    fn new(domain: &str) -> Self {
        let mut hasher = Hasher {
            inner: Sha256::new(),
        };
        hasher.string(domain);
        hasher
    }

    fn byte(&mut self, value: u8) {
        self.inner.update([value]);
    }

    fn boolean(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn uint64(&mut self, value: u64) {
        self.inner.update(value.to_le_bytes());
    }

    fn string(&mut self, value: &str) {
        self.uint64(value.len() as u64);
        self.inner.update(value.as_bytes());
    }

    fn digest(&mut self, value: &Shape) {
        self.inner.update(value);
    }

    fn type_ref(&mut self, reference: &TypeRef) {
        self.byte(reference.kind as u8);
        self.string(&reference.qname);
        self.boolean(reference.bare);
        self.boolean(reference.percent);
        self.boolean(reference.arg.is_some());
        if let Some(arg) = &reference.arg {
            self.type_ref(arg);
        }
    }

    fn wire_type_ref(&mut self, reference: &TypeRef, generic_params: &[(String, u64)]) {
        self.byte(reference.kind as u8);
        if reference.kind == TypeKind::GenericRef {
            let index = generic_params
                .iter()
                .find(|(name, _)| name == &reference.qname)
                .map(|(_, index)| *index)
                .unwrap_or(0);
            self.uint64(index);
        } else {
            self.string(reference.wire_name());
        }
        self.boolean(reference.bare);
        self.boolean(reference.percent);
        self.boolean(reference.arg.is_some());
        if let Some(arg) = &reference.arg {
            self.wire_type_ref(arg, generic_params);
        }
    }

    fn sum(&self) -> Shape {
        let mut out = [0u8; 32];
        out.copy_from_slice(&self.inner.clone().finalize());
        out
    }
}

fn body_shape(def: &Definition) -> Shape {
    let mut hasher = Hasher::new("gotd.tl.semantic.body.v1");
    hasher.boolean(def.base);
    hasher.uint64(def.generic_params.len() as u64);
    for param in &def.generic_params {
        hasher.string(param);
    }
    hasher.uint64(def.fields.len() as u64);
    for field in &def.fields {
        hasher.byte(field.kind as u8);
        hasher.string(&field.name);
        hasher.uint64(field.ordinal as u64);
        if field.kind == FieldKind::Value {
            hasher.type_ref(&field.ty);
        }
        hasher.boolean(field.condition.is_some());
        if let Some(condition) = &field.condition {
            hasher.string(&condition.word);
            hasher.byte(condition.bit);
            hasher.boolean(condition.presence_only);
        }
    }
    hasher.sum()
}

fn signature_shape(def: &Definition) -> Shape {
    let mut hasher = Hasher::new("gotd.tl.semantic.signature.v1");
    hasher.digest(&def.body_shape);
    hasher.type_ref(&def.result);
    hasher.sum()
}

fn payload_wire_shape(def: &Definition) -> Shape {
    let mut hasher = Hasher::new("gotd.tl.semantic.payload-wire.v1");
    hasher.boolean(def.base);
    let generic_params: Vec<(String, u64)> = def
        .generic_params
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index as u64))
        .collect();
    hasher.uint64(generic_params.len() as u64);
    let encoded_fields = def
        .fields
        .iter()
        .filter(|field| {
            field
                .condition
                .as_ref()
                .is_none_or(|condition| !condition.presence_only)
        })
        .count();
    hasher.uint64(encoded_fields as u64);
    let mut flag_slots: Vec<(String, u64)> = Vec::new();
    let mut encoded_slot = 0u64;
    for field in &def.fields {
        if field
            .condition
            .as_ref()
            .is_some_and(|condition| condition.presence_only)
        {
            continue;
        }
        if field.kind == FieldKind::FlagsWord {
            flag_slots.push((field.name.clone(), encoded_slot));
            hasher.byte(TypeKind::Primitive as u8);
            hasher.string("int32");
            hasher.boolean(false);
            hasher.boolean(false);
            hasher.boolean(false);
        } else {
            hasher.wire_type_ref(&field.ty, &generic_params);
        }
        hasher.boolean(field.condition.is_some());
        if let Some(condition) = &field.condition {
            let slot = flag_slots
                .iter()
                .find(|(name, _)| name == &condition.word)
                .map(|(_, slot)| *slot)
                .unwrap_or(0);
            hasher.uint64(slot);
            hasher.byte(condition.bit);
        }
        encoded_slot += 1;
    }
    hasher.sum()
}

#[cfg(test)]
mod tests {
    use super::{Definition, SemanticKey, build_schema, semantic_stable_id};
    use crate::parse::parse_schema;

    fn source(layer: i32) -> super::SourceRef {
        super::SourceRef {
            layer,
            repository: "r".to_owned(),
            commit: "c".to_owned(),
            blob: "b".to_owned(),
            path: "p".to_owned(),
            file: "f".to_owned(),
            sha256: [0; 32],
        }
    }

    #[test]
    fn semantic_id_is_category_qualified() {
        use sha2::Digest;

        let key = SemanticKey {
            category: crate::parse::Category::Type,
            qname: "foo".to_owned(),
        };
        let id = semantic_stable_id(&key);
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"gotd.tl.layer-semantic.v1\x00");
        hasher.update(b"type:foo");
        let digest = hasher.finalize();
        assert_eq!(id.to_le_bytes(), digest[..8]);
    }

    #[test]
    fn builds_and_groups() {
        let raw = parse_schema("a#1 x:int = Foo;\nb#2 y:int = Foo;\n").unwrap();
        let model = build_schema(&raw, source(232)).unwrap();
        assert_eq!(model.definitions.len(), 2);
        assert_eq!(model.class_constructors("Foo").len(), 2);
    }

    #[test]
    fn identical_definitions_share_shape() {
        let raw = parse_schema("a#1 x:int = Foo;\n").unwrap();
        let model = build_schema(&raw, source(232)).unwrap();
        let def: &Definition = &model.definitions[0];
        assert_eq!(def.body_shape, def.body_shape);
        assert_ne!(def.body_shape, def.wire_shape);
    }
}
