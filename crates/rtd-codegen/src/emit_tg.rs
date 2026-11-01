//! Emits the canonical `rtd-tg` value model for one schema.
//!
//! The output is a Rust-idiomatic model: one enum per TL class, one struct per
//! method, and a universal `Object`/`Method` enum for boxed dispatch. Encoding
//! and decoding go through `rtd-bin`'s `Writer`/`Reader`, wrapped in a bounded
//! [`Decoder`] that enforces admission limits while decoding.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Definition, Field, FieldKind, TypeKind, TypeRef};
use crate::parse::Category;

use crate::emit::{field_ident, ident_for_qname, to_upper_snake, variant_ident};

/// Returns the final component of a dotted qualified name.
fn local_name(definition: &Definition) -> String {
    definition
        .key
        .qname
        .rsplit('.')
        .next()
        .unwrap_or(&definition.key.qname)
        .to_owned()
}

/// A generated Rust source file.
pub(crate) struct EmitError(pub(crate) String);

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) struct Code {
    pub(crate) out: String,
    indent: usize,
}

impl Code {
    pub(crate) fn new() -> Self {
        Code {
            out: String::new(),
            indent: 0,
        }
    }

    pub(crate) fn line(&mut self, text: &str) {
        if text.is_empty() {
            self.out.push('\n');
            return;
        }
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    pub(crate) fn open(&mut self, text: &str) {
        self.line(text);
        self.indent += 1;
    }

    pub(crate) fn close(&mut self, text: &str) {
        self.indent = self.indent.saturating_sub(1);
        self.line(text);
    }
}

/// Maps a TL type to its Rust type and codec strategy.
#[derive(Debug, Clone)]
pub(crate) enum RustType {
    U32,
    I32,
    I64,
    F64,
    Int128,
    Int256,
    String,
    Bytes,
    Bool,
    Class(String),
    /// A bare constructor reference (`%name` / lowercase), encoded without its
    /// identifier.
    ClassBare(String, u32),
    /// The universal boxed `Object`.
    Object,
    Method,
    VectorBoxed(Box<RustType>),
    VectorBare(Box<RustType>),
}

impl RustType {
    pub(crate) fn rendered(&self) -> String {
        self.rendered_in(false)
    }

    /// Renders the type. When `qualify` is set, generated `rtd_tg` type names are
    /// prefixed, so the profile emitters can avoid importing them (a TL type is
    /// literally named `String`, which would shadow `std::string::String`).
    pub(crate) fn rendered_in(&self, qualify: bool) -> String {
        let name = |name: &str| -> String {
            if qualify {
                format!("rtd_tg::{name}")
            } else {
                name.to_owned()
            }
        };
        match self {
            RustType::U32 => "u32".to_owned(),
            RustType::I32 => "i32".to_owned(),
            RustType::I64 => "i64".to_owned(),
            RustType::F64 => "f64".to_owned(),
            RustType::Int128 => "::rtd_bin::Int128".to_owned(),
            RustType::Int256 => "::rtd_bin::Int256".to_owned(),
            RustType::String => "::std::string::String".to_owned(),
            RustType::Bytes => "::std::vec::Vec<u8>".to_owned(),
            RustType::Bool => "bool".to_owned(),
            RustType::Class(name_inner) => format!("::std::boxed::Box<{}>", name(name_inner)),
            RustType::ClassBare(name_inner, _) => name(name_inner),
            RustType::Object => format!("::std::boxed::Box<{}>", name("Object")),
            RustType::Method => format!("::std::boxed::Box<{}>", name("Method")),
            RustType::VectorBoxed(element) | RustType::VectorBare(element) => {
                format!(
                    "::std::vec::Vec<{}>",
                    element.rendered_vector_element_in(qualify)
                )
            }
        }
    }

    fn rendered_vector_element_in(&self, qualify: bool) -> String {
        let name = |name: &str| -> String {
            if qualify {
                format!("rtd_tg::{name}")
            } else {
                name.to_owned()
            }
        };
        match self {
            RustType::Class(name_inner) | RustType::ClassBare(name_inner, _) => name(name_inner),
            RustType::Method => name("Method"),
            RustType::Object => name("Object"),
            other => other.rendered_in(qualify),
        }
    }
}

pub(crate) fn map_type(
    reference: &TypeRef,
    classes: &BTreeSet<String>,
    bare_ids: &BTreeMap<String, (String, u32)>,
) -> Result<RustType, EmitError> {
    match reference.kind {
        TypeKind::GenericRef => Ok(RustType::Method),
        TypeKind::Primitive => Ok(match reference.qname.as_str() {
            "int" | "int32" | "Int" => RustType::I32,
            "int53" | "int64" | "long" | "Long" => RustType::I64,
            "double" | "Double" => RustType::F64,
            "int128" => RustType::Int128,
            "int256" => RustType::Int256,
            "string" | "String" => RustType::String,
            "bytes" | "Bytes" => RustType::Bytes,
            "bool" | "Bool" | "true" | "false" | "True" => RustType::Bool,
            "Object" => RustType::Object,
            other => return Err(EmitError(format!("unmapped primitive {other:?}"))),
        }),
        TypeKind::Named => {
            let ident = ident_for_qname(&reference.qname);
            if reference.bare || reference.percent {
                let (class_ident, id) = bare_ids.get(&reference.qname).ok_or_else(|| {
                    EmitError(format!(
                        "bare reference to multi-constructor class {:?}",
                        reference.qname
                    ))
                })?;
                return Ok(RustType::ClassBare(class_ident.clone(), *id));
            }
            if !classes.contains(&ident) {
                return Err(EmitError(format!(
                    "reference to unknown class {:?}",
                    reference.qname
                )));
            }
            Ok(RustType::Class(ident))
        }
        TypeKind::Vector => {
            let arg = reference
                .arg
                .as_ref()
                .ok_or_else(|| EmitError("vector without element".to_owned()))?;
            let element = map_type(arg, classes, bare_ids)?;
            if reference.qname == "vector" {
                Ok(RustType::VectorBare(::std::boxed::Box::new(element)))
            } else {
                Ok(RustType::VectorBoxed(::std::boxed::Box::new(element)))
            }
        }
    }
}

/// Emits the encoding of one value.
///
/// `place` is a place expression with no borrow or dereference of its own:
/// `self.x` for a struct field, or a match binding (`x`) otherwise. When
/// `by_ref` is set, `place` is already a reference and scalar values are
/// dereferenced rather than borrowed.
pub(crate) fn encode_value(
    code: &mut Code,
    ty: &RustType,
    place: &str,
    by_ref: bool,
    profile: bool,
) {
    let scalar = |code: &mut Code, method: &str| {
        if by_ref {
            code.line(&format!("__w.{method}(*{place});"));
        } else {
            code.line(&format!("__w.{method}({place});"));
        }
    };
    match ty {
        RustType::U32 => scalar(code, "put_u32"),
        RustType::I32 => scalar(code, "put_i32"),
        RustType::I64 => scalar(code, "put_i64"),
        RustType::F64 => scalar(code, "put_f64"),
        RustType::Int128 => scalar(code, "put_int128"),
        RustType::Int256 => scalar(code, "put_int256"),
        RustType::Bool => scalar(code, "put_bool"),
        RustType::String => code.line(&format!("__w.put_string({place}.as_str());")),
        RustType::Bytes => code.line(&format!("__w.put_bytes({place}.as_slice());")),
        RustType::Class(name) => {
            if profile {
                code.line(&format!(
                    "encode_{}_profile(profile, {place}, __w)?;",
                    crate::emit::to_snake_case(name)
                ));
            } else {
                code.line(&format!("{place}.encode(__w);"));
            }
        }
        RustType::ClassBare(name, _) => {
            if profile {
                code.line(&format!(
                    "encode_{}_profile(profile, {place}, __w)?;",
                    crate::emit::to_snake_case(name)
                ));
            } else {
                code.line(&format!("{place}.encode_body(__w);"));
            }
        }
        RustType::Object => {
            if profile {
                code.line(&format!("encode_object_profile(profile, {place}, __w)?;"));
            } else {
                code.line(&format!("{place}.encode(__w);"));
            }
        }
        RustType::Method => {
            if profile {
                code.line(&format!("encode_method_profile(profile, {place}, __w)?;"));
            } else {
                code.line(&format!("{place}.encode(__w);"));
            }
        }
        RustType::VectorBoxed(element) => {
            code.line(&format!("__w.put_vector_header({place}.len());"));
            code.open(&format!("for item in {place}.iter() {{"));
            encode_value(code, element, "item", true, profile);
            code.close("}");
        }
        RustType::VectorBare(element) => {
            code.line(&format!("__w.put_i32({place}.len() as i32);"));
            code.open(&format!("for item in {place}.iter() {{"));
            encode_value(code, element, "item", true, profile);
            code.close("}");
        }
    }
}

pub(crate) fn decode_expr(ty: &RustType, profile: bool) -> String {
    match ty {
        RustType::U32 => "__d.take_u32()?".to_owned(),
        RustType::I32 => "__d.take_i32()?".to_owned(),
        RustType::I64 => "__d.take_i64()?".to_owned(),
        RustType::F64 => "__d.take_f64()?".to_owned(),
        RustType::Int128 => "__d.take_int128()?".to_owned(),
        RustType::Int256 => "__d.take_int256()?".to_owned(),
        RustType::String => "__d.take_string()?.to_owned()".to_owned(),
        RustType::Bytes => "__d.take_bytes()?.to_vec()".to_owned(),
        RustType::Bool => "__d.take_bool()?".to_owned(),
        RustType::Class(name) => {
            if profile {
                format!(
                    "{{ __d.enter()?; let value = {{ let __id = __d.take_u32()?; decode_{}_profile(profile, __id, __d)? }}; __d.leave(); ::std::boxed::Box::new(value) }}",
                    crate::emit::to_snake_case(name)
                )
            } else {
                format!(
                    "{{ __d.enter()?; let value = {name}::decode(__d)?; __d.leave(); ::std::boxed::Box::new(value) }}"
                )
            }
        }
        RustType::ClassBare(name, id) => {
            if profile {
                format!(
                    "{{ __d.enter()?; let value = {{ let __id = __d.take_u32()?; decode_{}_profile(profile, __id, __d)? }}; __d.leave(); value }}",
                    crate::emit::to_snake_case(name)
                )
            } else {
                format!(
                    "{{ __d.enter()?; let value = {name}::decode_with_id(__d, {id:#010x})?; __d.leave(); value }}"
                )
            }
        }
        RustType::Object => {
            if profile {
                "{ __d.enter()?; let value = decode_object_profile(profile, __d)?; __d.leave(); ::std::boxed::Box::new(value) }".to_owned()
            } else {
                "{ __d.enter()?; let value = Object::decode(__d)?; __d.leave(); ::std::boxed::Box::new(value) }".to_owned()
            }
        }
        RustType::Method => {
            if profile {
                "{ __d.enter()?; let value = decode_method_profile(profile, __d)?; __d.leave(); ::std::boxed::Box::new(value) }".to_owned()
            } else {
                "{ __d.enter()?; let value = Method::decode(__d)?; __d.leave(); ::std::boxed::Box::new(value) }".to_owned()
            }
        }
        RustType::VectorBoxed(element) => {
            let element = decode_element(element, profile);
            format!(
                "{{ let n = __d.take_vector_header()?; let mut items = ::std::vec::Vec::with_capacity(::rtd_bin::preallocate_len(n)); for _ in 0..n {{ items.push({element}); }} items }}"
            )
        }
        RustType::VectorBare(element) => {
            let element = decode_element(element, profile);
            format!(
                "{{ let n = __d.take_bare_vector_len()?; let mut items = ::std::vec::Vec::with_capacity(::rtd_bin::preallocate_len(n)); for _ in 0..n {{ items.push({element}); }} items }}"
            )
        }
    }
}

pub(crate) fn decode_element(ty: &RustType, profile: bool) -> String {
    match ty {
        RustType::Class(name) => {
            if profile {
                format!(
                    "{{ __d.enter()?; let value = {{ let __id = __d.take_u32()?; decode_{}_profile(profile, __id, __d)? }}; __d.leave(); value }}",
                    crate::emit::to_snake_case(name)
                )
            } else {
                format!("{{ __d.enter()?; let value = {name}::decode(__d)?; __d.leave(); value }}")
            }
        }
        RustType::ClassBare(name, id) => {
            if profile {
                format!(
                    "{{ __d.enter()?; let value = {{ let __id = __d.take_u32()?; decode_{}_profile(profile, __id, __d)? }}; __d.leave(); value }}",
                    crate::emit::to_snake_case(name)
                )
            } else {
                format!(
                    "{{ __d.enter()?; let value = {name}::decode_with_id(__d, {id:#010x})?; __d.leave(); value }}"
                )
            }
        }
        RustType::Object => {
            if profile {
                "{ __d.enter()?; let value = decode_object_profile(profile, __d)?; __d.leave(); value }".to_owned()
            } else {
                "{ __d.enter()?; let value = Object::decode(__d)?; __d.leave(); value }".to_owned()
            }
        }
        RustType::Method => {
            if profile {
                "{ __d.enter()?; let value = decode_method_profile(profile, __d)?; __d.leave(); value }".to_owned()
            } else {
                "{ __d.enter()?; let value = Method::decode(__d)?; __d.leave(); value }".to_owned()
            }
        }
        other => decode_expr(other, profile),
    }
}

pub(crate) fn field_rust_type(
    field: &Field,
    classes: &BTreeSet<String>,
    bare_ids: &BTreeMap<String, (String, u32)>,
) -> Result<RustType, EmitError> {
    if field.kind == FieldKind::FlagsWord {
        return Ok(RustType::U32);
    }
    map_type(&field.ty, classes, bare_ids)
}

pub(crate) fn flags_mask(fields: &[Field], word: &str) -> u32 {
    let mut mask = 0u32;
    for field in fields {
        if let Some(condition) = &field.condition
            && condition.word == word
        {
            mask |= 1u32 << condition.bit;
        }
    }
    mask
}

pub(crate) fn emit_constructor_decode(
    code: &mut Code,
    fields: &[Field],
    types: &[RustType],
    construct: &str,
    profile: bool,
) {
    for (index, field) in fields.iter().enumerate() {
        let binding = field_ident(&field.name);
        let ty = &types[index];
        if field.kind == FieldKind::FlagsWord {
            code.line(&format!("let {binding} = __d.take_u32()?;"));
            continue;
        }
        match &field.condition {
            None => code.line(&format!(
                "let {binding}: {} = {};",
                ty.rendered_in(profile),
                decode_expr(ty, profile)
            )),
            Some(condition) if condition.presence_only => code.line(&format!(
                "let {binding}: bool = {} & (1u32 << {}) != 0;",
                field_ident(&condition.word),
                condition.bit
            )),
            Some(condition) => code.line(&format!(
                "let {binding}: Option<{}> = if {} & (1u32 << {}) != 0 {{ Some({}) }} else {{ None }};",
                ty.rendered_in(profile),
                field_ident(&condition.word),
                condition.bit,
                decode_expr(ty, profile)
            )),
        }
    }
    let bindings: Vec<String> = fields
        .iter()
        .map(|field| field_ident(&field.name))
        .collect();
    if bindings.is_empty() {
        code.line(&format!("Ok({construct})"));
    } else {
        code.line(&format!("Ok({construct} {{ {} }})", bindings.join(", ")));
    }
}

fn emit_variant_fields(code: &mut Code, fields: &[Field], types: &[RustType], is_struct: bool) {
    let prefix = if is_struct { "pub " } else { "" };
    for (index, field) in fields.iter().enumerate() {
        let ty = &types[index];
        let binding = field_ident(&field.name);
        match &field.condition {
            None => code.line(&format!("{prefix}{binding}: {},", ty.rendered())),
            Some(condition) if condition.presence_only => {
                code.line(&format!("{prefix}{binding}: bool,"));
            }
            Some(_) => code.line(&format!("{prefix}{binding}: Option<{}>,", ty.rendered())),
        }
    }
}

pub(crate) fn prepare(
    definition: &Definition,
    classes: &BTreeSet<String>,
    bare_ids: &BTreeMap<String, (String, u32)>,
) -> Result<Vec<RustType>, EmitError> {
    definition
        .fields
        .iter()
        .map(|field| field_rust_type(field, classes, bare_ids))
        .collect()
}

/// A schema's class map, class set and bare-constructor map.
pub(crate) type ClassContext<'a> = (
    BTreeMap<String, Vec<&'a Definition>>,
    BTreeSet<String>,
    BTreeMap<String, (String, u32)>,
);

/// Computes the class map, class set and bare-constructor map for a schema.
pub(crate) fn class_context(schema: &crate::model::SchemaModel) -> ClassContext<'_> {
    let mut class_map: BTreeMap<String, Vec<&Definition>> = BTreeMap::new();
    for definition in &schema.definitions {
        if definition.key.category != Category::Type || definition.result.qname == "Object" {
            continue;
        }
        class_map
            .entry(definition.result.qname.clone())
            .or_default()
            .push(definition);
    }
    let classes: BTreeSet<String> = class_map.keys().map(|name| ident_for_qname(name)).collect();
    let mut bare_ids: BTreeMap<String, (String, u32)> = BTreeMap::new();
    for (qname, constructors) in &class_map {
        if constructors.len() == 1 {
            let ident = ident_for_qname(qname);
            bare_ids.insert(qname.clone(), (ident.clone(), constructors[0].wire_id));
            bare_ids.insert(
                constructors[0].key.qname.clone(),
                (ident, constructors[0].wire_id),
            );
        }
    }
    (class_map, classes, bare_ids)
}

/// Emits the full `rtd-tg` source for one canonical schema.
///
/// # Errors
///
/// Returns an error when a type cannot be mapped or generated names collide.
pub(crate) fn emit(schema: &crate::model::SchemaModel) -> Result<String, EmitError> {
    let object_constructors: Vec<&Definition> = schema
        .definitions
        .iter()
        .filter(|definition| {
            definition.key.category == Category::Type && definition.result.qname == "Object"
        })
        .collect();
    let methods: Vec<&Definition> = schema
        .definitions
        .iter()
        .filter(|definition| definition.key.category == Category::Function)
        .collect();

    // A class with exactly one constructor can be referenced bare, either by
    // its class name or by the constructor name.
    let (class_map, classes, bare_ids) = class_context(schema);

    // Detect identifier collisions early and deterministically.
    let mut used: BTreeMap<String, String> = BTreeMap::new();
    for class in class_map.keys() {
        let ident = ident_for_qname(class);
        if let Some(previous) = used.insert(ident.clone(), class.clone()) {
            return Err(EmitError(format!(
                "class identifier {ident:?} collides between {previous:?} and {class:?}"
            )));
        }
    }
    for definition in object_constructors.iter().chain(methods.iter()) {
        let ident = ident_for_qname(&definition.key.qname);
        if let Some(previous) = used.insert(ident.clone(), definition.key.qname.clone()) {
            return Err(EmitError(format!(
                "identifier {ident:?} collides between {previous:?} and {:?}",
                definition.key.qname
            )));
        }
    }

    let mut code = Code::new();
    code.line("// @generated by rtd-codegen. Do not edit by hand.");
    // Distinct TL names can differ only by case (`RpcDropAnswer` is a type and
    // `rpc_drop_answer` a method), so the generated spelling is preserved.
    code.line("// TL names keep their original spelling; distinct names may differ only by case.");
    code.line("#![allow(non_camel_case_types)]");
    code.line("");
    code.line("//! Generated canonical Telegram API value model.");
    code.line("");
    code.line("/// The canonical schema layer.");
    code.line(&format!("pub const LAYER: i32 = {};", schema.layer));
    code.line("");
    emit_decoder(&mut code);

    // Classes.
    let mut class_names: Vec<&String> = class_map.keys().collect();
    class_names.sort();
    for class_qname in class_names {
        let class_ident = ident_for_qname(class_qname);
        let mut constructors: Vec<&Definition> = class_map[class_qname].clone();
        constructors.sort_by(|a, b| a.key.qname.cmp(&b.key.qname));
        // A type whose single constructor is named after the type itself is a
        // concrete type, not a union: emit a struct rather than a one-variant
        // enum. Real unions (several constructors, or a differently named one)
        // stay enums.
        let is_concrete = constructors.len() == 1 && {
            let ctor_qname = &constructors[0].key.qname;
            let type_local = class_qname.rsplit('.').next().unwrap_or(class_qname);
            let ctor_local = ctor_qname.rsplit('.').next().unwrap_or(ctor_qname);
            type_local.eq_ignore_ascii_case(ctor_local)
        };
        if is_concrete {
            emit_struct(
                &mut code,
                &class_ident,
                constructors[0],
                &classes,
                &bare_ids,
            )?;
        } else {
            emit_class_enum(&mut code, &class_ident, &constructors, &classes, &bare_ids)?;
        }
    }

    // Direct `Object` constructors (for example `gzip_packed`).
    for definition in &object_constructors {
        let ident = ident_for_qname(&definition.key.qname);
        emit_struct(&mut code, &ident, definition, &classes, &bare_ids)?;
    }

    // Methods.
    for method in &methods {
        let ident = ident_for_qname(&method.key.qname);
        emit_struct(&mut code, &ident, method, &classes, &bare_ids)?;
    }

    emit_object(
        &mut code,
        &class_map,
        &object_constructors,
        &classes,
        &bare_ids,
    )?;
    emit_method_enum(&mut code, &methods)?;
    emit_type_id_trait(&mut code, &class_map, &object_constructors, &methods);
    emit_ids(&mut code, schema);
    emit_types_map(&mut code, schema);

    // A generated smoke test: the crate compiles and exposes its registry.
    code.line("#[cfg(test)]");
    code.open("mod generated_tests {");
    code.open("use super::{LAYER, types_map};");
    code.line("#[test]");
    code.open("fn registry_is_sorted_and_non_empty() {");
    code.line("assert!(!types_map().is_empty());");
    code.line("assert!(types_map().windows(2).all(|w| w[0].0 <= w[1].0));");
    code.line("let _ = LAYER;");
    code.close("}");
    code.close("}");

    Ok(code.out)
}

fn emit_class_enum(
    code: &mut Code,
    class_ident: &str,
    constructors: &[&Definition],
    classes: &BTreeSet<String>,
    bare_ids: &BTreeMap<String, (String, u32)>,
) -> Result<(), EmitError> {
    let mut variants = Vec::new();
    for constructor in constructors {
        let types = prepare(constructor, classes, bare_ids)?;
        variants.push((constructor, types));
    }
    // Constructors of a single TL class differ widely in field count, so the
    // largest variant can dwarf the others. Boxing a variant would change the
    // public model for every consumer, so the size lint is accepted for
    // generated class enums only (never for hand-written code).
    code.line("#[allow(clippy::large_enum_variant)]");
    code.line("#[derive(Debug, Clone, PartialEq)]");
    code.open(&format!("pub enum {class_ident} {{"));
    for (constructor, types) in &variants {
        let variant = variant_ident(&local_name(constructor));
        if constructor.fields.is_empty() {
            code.line(&format!("{variant},"));
        } else {
            code.open(&format!("{variant} {{"));
            emit_variant_fields(code, &constructor.fields, types, false);
            code.close("},");
        }
    }
    code.close("}");

    code.open(&format!("impl {class_ident} {{"));
    // type_id
    code.open("/// Returns the wire constructor identifier of this value.");
    code.open("pub fn type_id(&self) -> u32 {");
    code.open("match self {");
    for (constructor, _) in &variants {
        let variant = variant_ident(&local_name(constructor));
        let pattern = if constructor.fields.is_empty() {
            format!("{class_ident}::{variant}")
        } else {
            format!("{class_ident}::{variant} {{ .. }}")
        };
        code.line(&format!("{pattern} => {:#010x},", constructor.wire_id));
    }
    code.close("}");
    code.close("}");
    code.close("");
    // encode
    code.open("/// Encodes the value, including its constructor identifier.");
    code.open("pub fn encode(&self, __w: &mut ::rtd_bin::Writer) {");
    code.open("match self {");
    for (constructor, types) in &variants {
        let variant = variant_ident(&local_name(constructor));
        if constructor.fields.is_empty() {
            code.line(&format!("{class_ident}::{variant} => {{"));
            code.indent += 1;
            code.line(&format!("__w.put_u32({:#010x});", constructor.wire_id));
            code.indent -= 1;
            code.line("}");
        } else {
            let bindings: Vec<String> = constructor
                .fields
                .iter()
                .map(|field| field_ident(&field.name))
                .collect();
            code.open(&format!(
                "{class_ident}::{variant} {{ {} }} => {{",
                bindings.join(", ")
            ));
            code.line(&format!("__w.put_u32({:#010x});", constructor.wire_id));
            emit_encode_fields(code, &constructor.fields, types, false, false);
            code.close("}");
        }
    }
    code.close("}");
    code.close("}");
    code.close("");
    // encode_body
    code.open("/// Encodes the value without its constructor identifier.");
    code.open("pub fn encode_body(&self, __w: &mut ::rtd_bin::Writer) {");
    code.open("match self {");
    for (constructor, types) in &variants {
        let variant = variant_ident(&local_name(constructor));
        if constructor.fields.is_empty() {
            code.line(&format!("{class_ident}::{variant} => {{}}"));
        } else {
            let bindings: Vec<String> = constructor
                .fields
                .iter()
                .map(|field| field_ident(&field.name))
                .collect();
            code.open(&format!(
                "{class_ident}::{variant} {{ {} }} => {{",
                bindings.join(", ")
            ));
            emit_encode_fields(code, &constructor.fields, types, false, false);
            code.close("}");
        }
    }
    code.close("}");
    code.close("}");
    code.close("");
    // decode
    code.open("/// Decodes a value, reading and dispatching on its identifier.");
    code.open("pub fn decode(__d: &mut Decoder<'_>) -> ::rtd_bin::Result<Self> {");
    code.line("let id = __d.take_u32()?;");
    code.line("Self::decode_with_id(__d, id)");
    code.close("}");
    code.close("");
    code.open("/// Decodes a value whose identifier has already been read.");
    code.open("pub fn decode_with_id(__d: &mut Decoder<'_>, id: u32) -> ::rtd_bin::Result<Self> {");
    code.open("match id {");
    for (constructor, types) in &variants {
        let variant = variant_ident(&local_name(constructor));
        code.open(&format!("{:#010x} => {{", constructor.wire_id));
        emit_constructor_decode(
            code,
            &constructor.fields,
            types,
            &format!("{class_ident}::{variant}"),
            false,
        );
        code.close("}");
    }
    let expected = variants
        .first()
        .map(|(constructor, _)| constructor.wire_id)
        .unwrap_or(0);
    code.line(&format!(
        "other => Err(::rtd_bin::Error::UnexpectedId {{ id: other, expected: {expected:#010x} }}),"
    ));
    code.close("}");
    code.close("}");
    code.close("");
    code.close("}");
    Ok(())
}

fn emit_struct(
    code: &mut Code,
    ident: &str,
    method: &Definition,
    classes: &BTreeSet<String>,
    bare_ids: &BTreeMap<String, (String, u32)>,
) -> Result<(), EmitError> {
    let types = prepare(method, classes, bare_ids)?;
    code.line("#[derive(Debug, Clone, PartialEq)]");
    if method.fields.is_empty() {
        code.line(&format!("pub struct {ident};"));
    } else {
        code.open(&format!("pub struct {ident} {{"));
        emit_variant_fields(code, &method.fields, &types, true);
        code.close("}");
    }
    code.open(&format!("impl {ident} {{"));
    code.line("/// The wire constructor identifier.");
    code.line(&format!(
        "pub const TYPE_ID: u32 = {:#010x};",
        method.wire_id
    ));
    code.open("/// Returns the wire constructor identifier of this value.");
    code.open("pub fn type_id(&self) -> u32 {");
    code.line("Self::TYPE_ID");
    code.close("}");
    code.close("");
    code.open("/// Encodes the value, including its constructor identifier.");
    code.open("pub fn encode(&self, __w: &mut ::rtd_bin::Writer) {");
    code.line(&format!("__w.put_u32({:#010x});", method.wire_id));
    if !method.fields.is_empty() {
        emit_encode_fields(code, &method.fields, &types, true, false);
    }
    code.close("}");
    code.close("");
    code.open("/// Encodes the fields without the constructor identifier.");
    code.open("pub fn encode_body(&self, __w: &mut ::rtd_bin::Writer) {");
    if !method.fields.is_empty() {
        emit_encode_fields(code, &method.fields, &types, true, false);
    }
    code.close("}");
    code.close("");
    code.open("/// Decodes a value with an already-read identifier.");
    code.open("pub fn decode_with_id(__d: &mut Decoder<'_>, id: u32) -> ::rtd_bin::Result<Self> {");
    code.open(&format!("if id != {:#010x} {{", method.wire_id));
    code.line(&format!(
        "return Err(::rtd_bin::Error::UnexpectedId {{ id, expected: {:#010x} }});",
        method.wire_id
    ));
    code.close("}");
    if method.fields.is_empty() {
        code.line(&format!("Ok({ident})"));
    } else {
        emit_constructor_decode(code, &method.fields, &types, ident, false);
    }
    code.close("}");
    code.close("");
    code.open("/// Decodes a value, reading and checking its identifier.");
    code.open("pub fn decode(__d: &mut Decoder<'_>) -> ::rtd_bin::Result<Self> {");
    code.line("let id = __d.take_u32()?;");
    code.line("Self::decode_with_id(__d, id)");
    code.close("}");
    code.close("");
    code.close("}");
    Ok(())
}

/// Emits field encoding. `is_struct` selects `self.<field>` access; otherwise
/// the match-arm bindings are used directly.
pub(crate) fn emit_encode_fields(
    code: &mut Code,
    fields: &[Field],
    types: &[RustType],
    is_struct: bool,
    profile: bool,
) {
    // Struct fields are reached through `self`; match-arm bindings are already
    // references, so scalar fields must be dereferenced there.
    let by_ref = !is_struct;
    let place = |field: &Field| -> String {
        if is_struct {
            format!("self.{}", field_ident(&field.name))
        } else {
            field_ident(&field.name)
        }
    };
    let value = |field: &Field| -> String {
        if by_ref {
            format!("*{}", place(field))
        } else {
            place(field)
        }
    };
    for (index, field) in fields.iter().enumerate() {
        if field.kind != FieldKind::FlagsWord {
            continue;
        }
        let local = format!("__flags_{}", field.name);
        let mask = flags_mask(fields, &field.name);
        if mask == 0 {
            code.line(&format!("let mut {local} = {};", value(field)));
        } else {
            code.line(&format!(
                "let mut {local} = {} & !{mask:#010x}u32;",
                value(field)
            ));
        }
        for (other_index, other) in fields.iter().enumerate() {
            if let Some(condition) = &other.condition
                && condition.word == field.name
                && other_index != index
            {
                if condition.presence_only {
                    code.line(&format!(
                        "if {} {{ {local} |= 1u32 << {}; }}",
                        value(other),
                        condition.bit
                    ));
                } else {
                    code.line(&format!(
                        "if {}.is_some() {{ {local} |= 1u32 << {}; }}",
                        place(other),
                        condition.bit
                    ));
                }
            }
        }
        code.line(&format!("__w.put_u32({local});"));
    }
    for (index, field) in fields.iter().enumerate() {
        if field.kind == FieldKind::FlagsWord {
            continue;
        }
        let ty = &types[index];
        match &field.condition {
            None => encode_value(code, ty, &place(field), by_ref, profile),
            Some(condition) if condition.presence_only => {
                let _ = condition;
            }
            Some(_) => {
                // Borrow in struct mode so the `Option` is not moved out of `self`.
                let option_place = if is_struct {
                    format!("&{}", place(field))
                } else {
                    place(field)
                };
                code.open(&format!("if let Some(value) = {option_place} {{"));
                encode_value(code, ty, "value", true, profile);
                code.close("}");
            }
        }
    }
}

fn emit_object(
    code: &mut Code,
    class_map: &BTreeMap<String, Vec<&Definition>>,
    object_constructors: &[&Definition],
    classes: &BTreeSet<String>,
    bare_ids: &BTreeMap<String, (String, u32)>,
) -> Result<(), EmitError> {
    let _ = (classes, bare_ids);
    let mut class_names: Vec<&String> = class_map.keys().collect();
    class_names.sort();
    let mut object_names: Vec<String> = object_constructors
        .iter()
        .map(|definition| ident_for_qname(&definition.key.qname))
        .collect();
    object_names.sort();
    code.line("#[derive(Debug, Clone, PartialEq)]");
    code.open("pub enum Object {");
    for class_qname in &class_names {
        let ident = ident_for_qname(class_qname);
        code.line(&format!("{ident}(Box<{ident}>),"));
    }
    for ident in &object_names {
        code.line(&format!("{ident}(Box<{ident}>),"));
    }
    code.close("}");
    code.open("impl Object {");
    code.open("/// Returns the wire constructor identifier of the contained value.");
    code.open("pub fn type_id(&self) -> u32 {");
    code.open("match self {");
    for class_qname in &class_names {
        let ident = ident_for_qname(class_qname);
        code.line(&format!("Object::{ident}(value) => value.type_id(),"));
    }
    for ident in &object_names {
        code.line(&format!("Object::{ident}(value) => value.type_id(),"));
    }
    code.close("}");
    code.close("}");
    code.close("");
    code.open("/// Encodes the contained value.");
    code.open("pub fn encode(&self, __w: &mut ::rtd_bin::Writer) {");
    code.open("match self {");
    for class_qname in &class_names {
        let ident = ident_for_qname(class_qname);
        code.line(&format!("Object::{ident}(value) => value.encode(__w),"));
    }
    for ident in &object_names {
        code.line(&format!("Object::{ident}(value) => value.encode(__w),"));
    }
    code.close("}");
    code.close("}");
    code.close("");
    code.open("/// Encodes the contained value without its constructor identifier.");
    code.open("pub fn encode_body(&self, __w: &mut ::rtd_bin::Writer) {");
    code.open("match self {");
    for class_qname in &class_names {
        let ident = ident_for_qname(class_qname);
        code.line(&format!(
            "Object::{ident}(value) => value.encode_body(__w),"
        ));
    }
    for ident in &object_names {
        code.line(&format!(
            "Object::{ident}(value) => value.encode_body(__w),"
        ));
    }
    code.close("}");
    code.close("}");
    code.close("");
    code.open("/// Decodes a boxed value, reading and dispatching on its identifier.");
    code.open("pub fn decode(__d: &mut Decoder<'_>) -> ::rtd_bin::Result<Self> {");
    code.line("let id = __d.take_u32()?;");
    code.line("Self::decode_with_id(__d, id)");
    code.close("}");
    code.close("");
    code.open("/// Decodes a boxed value whose identifier has already been read.");
    code.open("pub fn decode_with_id(__d: &mut Decoder<'_>, id: u32) -> ::rtd_bin::Result<Self> {");
    code.open("match id {");
    let mut entries: Vec<(u32, String)> = Vec::new();
    for definition in class_map.values().flatten() {
        let ident = ident_for_qname(&definition.result.qname);
        entries.push((definition.wire_id, ident));
    }
    for definition in object_constructors {
        let ident = ident_for_qname(&definition.key.qname);
        entries.push((definition.wire_id, ident));
    }
    entries.sort();
    entry_guard(&entries);
    let mut expected = 0;
    for (wire_id, ident) in &entries {
        if expected == 0 {
            expected = *wire_id;
        }
        code.line(&format!(
            "{wire_id:#010x} => Ok(Object::{ident}(::std::boxed::Box::new({ident}::decode_with_id(__d, id)?))),"
        ));
    }
    code.line(&format!(
        "other => Err(::rtd_bin::Error::UnexpectedId {{ id: other, expected: {expected:#010x} }}),"
    ));
    code.close("}");
    code.close("}");
    code.close("");
    code.close("}");
    Ok(())
}

/// Rejects duplicate constructor identifiers deterministically.
fn entry_guard(entries: &[(u32, String)]) {
    // Duplicate identifiers within a schema are already rejected by the model;
    // this keeps the generated `match` valid if that ever changes.
    for window in entries.windows(2) {
        debug_assert_ne!(window[0].0, window[1].0);
    }
}

fn emit_method_enum(code: &mut Code, methods: &[&Definition]) -> Result<(), EmitError> {
    code.line("#[derive(Debug, Clone, PartialEq)]");
    code.open("pub enum Method {");
    for method in methods {
        let ident = ident_for_qname(&method.key.qname);
        code.line(&format!("{ident}(Box<{ident}>),"));
    }
    code.close("}");
    code.open("impl Method {");
    code.open("/// Returns the wire constructor identifier of the contained method.");
    code.open("pub fn type_id(&self) -> u32 {");
    code.open("match self {");
    for method in methods {
        let ident = ident_for_qname(&method.key.qname);
        code.line(&format!("Method::{ident}(value) => value.type_id(),"));
    }
    code.close("}");
    code.close("}");
    code.close("");
    code.open("/// Encodes the contained method.");
    code.open("pub fn encode(&self, __w: &mut ::rtd_bin::Writer) {");
    code.open("match self {");
    for method in methods {
        let ident = ident_for_qname(&method.key.qname);
        code.line(&format!("Method::{ident}(value) => value.encode(__w),"));
    }
    code.close("}");
    code.close("}");
    code.close("");
    code.open("/// Encodes the contained method without its constructor identifier.");
    code.open("pub fn encode_body(&self, __w: &mut ::rtd_bin::Writer) {");
    code.open("match self {");
    for method in methods {
        let ident = ident_for_qname(&method.key.qname);
        code.line(&format!(
            "Method::{ident}(value) => value.encode_body(__w),"
        ));
    }
    code.close("}");
    code.close("}");
    code.close("");
    code.open("/// Decodes a boxed method, reading and dispatching on its identifier.");
    code.open("pub fn decode(__d: &mut Decoder<'_>) -> ::rtd_bin::Result<Self> {");
    code.line("let id = __d.take_u32()?;");
    code.line("Self::decode_with_id(__d, id)");
    code.close("}");
    code.close("");
    code.open("/// Decodes a boxed method whose identifier has already been read.");
    code.open("pub fn decode_with_id(__d: &mut Decoder<'_>, id: u32) -> ::rtd_bin::Result<Self> {");
    code.open("match id {");
    let mut expected = 0;
    for method in methods {
        let ident = ident_for_qname(&method.key.qname);
        if expected == 0 {
            expected = method.wire_id;
        }
        code.line(&format!(
            "{:#010x} => Ok(Method::{ident}(::std::boxed::Box::new({ident}::decode_with_id(__d, id)?))),",
            method.wire_id
        ));
    }
    code.line(&format!(
        "other => Err(::rtd_bin::Error::UnexpectedId {{ id: other, expected: {expected:#010x} }}),"
    ));
    code.close("}");
    code.close("}");
    code.close("");
    code.close("}");
    Ok(())
}

/// Emits a `TypeId` trait and implementations for every generated value type.
fn emit_type_id_trait(
    code: &mut Code,
    class_map: &BTreeMap<String, Vec<&Definition>>,
    object_constructors: &[&Definition],
    methods: &[&Definition],
) {
    code.line("/// A wire constructor identifier accessor.");
    code.open("pub trait TypeId {");
    code.line("/// Returns the wire constructor identifier.");
    code.line("fn type_id(&self) -> u32;");
    code.close("}");
    let mut names: Vec<String> = class_map
        .keys()
        .map(|qname| ident_for_qname(qname))
        .collect();
    names.extend(
        object_constructors
            .iter()
            .map(|definition| ident_for_qname(&definition.key.qname)),
    );
    names.extend(
        methods
            .iter()
            .map(|definition| ident_for_qname(&definition.key.qname)),
    );
    names.push("Object".to_owned());
    names.push("Method".to_owned());
    names.sort();
    names.dedup();
    for name in names {
        code.open(&format!("impl TypeId for {name} {{"));
        code.open("fn type_id(&self) -> u32 {");
        code.line(&format!("{name}::type_id(self)"));
        code.close("}");
        code.close("}");
    }
}

fn emit_ids(code: &mut Code, schema: &crate::model::SchemaModel) {
    code.open("/// Wire constructor identifiers, keyed by name.");
    code.open("pub mod ids {");
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut entries: Vec<(String, u32)> = Vec::new();
    for definition in &schema.definitions {
        let mut name = to_upper_snake(&definition.key.qname);
        if !used.insert(name.clone()) {
            name = format!("{name}_{:08x}", definition.wire_id);
            used.insert(name.clone());
        }
        entries.push((name, definition.wire_id));
    }
    entries.sort();
    for (name, wire_id) in entries {
        code.line(&format!("pub const {name}: u32 = {wire_id:#010x};"));
    }
    code.close("}");
    code.close("");
}

fn emit_types_map(code: &mut Code, schema: &crate::model::SchemaModel) {
    let mut entries: Vec<(u32, String)> = schema
        .definitions
        .iter()
        .map(|definition| (definition.wire_id, definition.key.qname.clone()))
        .collect();
    entries.sort();
    entries.dedup();
    code.open("/// The wire-id to qualified-name registry, sorted by identifier.");
    code.open("pub fn types_map() -> &'static [(u32, &'static str)] {");
    code.open("&[");
    for (wire_id, qname) in &entries {
        code.line(&format!("({wire_id:#010x}, \"{qname}\"),"));
    }
    code.close("]");
    code.close("}");
    code.close("");
}

fn emit_decoder(code: &mut Code) {
    code.line("/// Decode limits enforced while materializing a value.");
    code.line("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    code.open("pub struct DecodeLimits {");
    code.line("/// Maximum elements in a single vector.");
    code.line("pub max_vector_elements: usize,");
    code.line("/// Maximum aggregate vector elements across the value.");
    code.line("pub max_aggregate_elements: usize,");
    code.line("/// Maximum nesting depth of boxed values.");
    code.line("pub max_depth: usize,");
    code.close("}");
    code.line("");
    code.open("impl Default for DecodeLimits {");
    code.open("fn default() -> Self {");
    code.open("Self {");
    code.line("max_vector_elements: 1 << 20,");
    code.line("max_aggregate_elements: 1 << 20,");
    code.line("max_depth: 64,");
    code.close("}");
    code.close("}");
    code.close("}");
    code.line("");
    code.line("/// A bounded cursor over borrowed TL bytes.");
    code.line("#[derive(Debug)]");
    code.open("pub struct Decoder<'a> {");
    code.line("reader: ::rtd_bin::Reader<'a>,");
    code.line("limits: DecodeLimits,");
    code.line("aggregate: usize,");
    code.line("depth: usize,");
    code.close("}");
    code.line("");
    code.open("impl<'a> Decoder<'a> {");
    code.line("/// Creates a bounded decoder over `input`.");
    code.open("pub fn new(input: &'a [u8], limits: DecodeLimits) -> Self {");
    code.line("Self { reader: ::rtd_bin::Reader::new(input), limits, aggregate: 0, depth: 0 }");
    code.close("}");
    code.line("");
    code.line("/// Returns the number of unread bytes.");
    code.open("pub fn remaining(&self) -> usize {");
    code.line("self.reader.remaining()");
    code.close("}");
    code.line("");
    code.line("/// Returns `true` when no bytes remain.");
    code.open("pub fn is_empty(&self) -> bool {");
    code.line("self.reader.is_empty()");
    code.close("}");
    code.line("");
    code.line("/// Returns the current cursor position.");
    code.open("pub fn position(&self) -> usize {");
    code.line("self.reader.position()");
    code.close("}");
    code.line("");
    code.line("/// Reads a 32-bit unsigned integer.");
    code.open("pub fn take_u32(&mut self) -> ::rtd_bin::Result<u32> {");
    code.line("self.reader.take_u32()");
    code.close("}");
    code.line("");
    code.line("/// Reads a 32-bit signed integer.");
    code.open("pub fn take_i32(&mut self) -> ::rtd_bin::Result<i32> {");
    code.line("self.reader.take_i32()");
    code.close("}");
    code.line("");
    code.line("/// Reads a 64-bit signed integer.");
    code.open("pub fn take_i64(&mut self) -> ::rtd_bin::Result<i64> {");
    code.line("self.reader.take_i64()");
    code.close("}");
    code.line("");
    code.line("/// Reads a 64-bit float.");
    code.open("pub fn take_f64(&mut self) -> ::rtd_bin::Result<f64> {");
    code.line("self.reader.take_f64()");
    code.close("}");
    code.line("");
    code.line("/// Reads a TL boolean.");
    code.open("pub fn take_bool(&mut self) -> ::rtd_bin::Result<bool> {");
    code.line("self.reader.take_bool()");
    code.close("}");
    code.line("");
    code.line("/// Reads a TL string.");
    code.open("pub fn take_string(&mut self) -> ::rtd_bin::Result<&'a str> {");
    code.line("self.reader.take_string()");
    code.close("}");
    code.line("");
    code.line("/// Reads a TL byte string.");
    code.open("pub fn take_bytes(&mut self) -> ::rtd_bin::Result<&'a [u8]> {");
    code.line("self.reader.take_bytes()");
    code.close("}");
    code.line("");
    code.line("/// Reads a 128-bit integer.");
    code.open("pub fn take_int128(&mut self) -> ::rtd_bin::Result<::rtd_bin::Int128> {");
    code.line("self.reader.take_int128()");
    code.close("}");
    code.line("");
    code.line("/// Reads a 256-bit integer.");
    code.open("pub fn take_int256(&mut self) -> ::rtd_bin::Result<::rtd_bin::Int256> {");
    code.line("self.reader.take_int256()");
    code.close("}");
    code.line("");
    code.line("/// Reads and bounds a boxed vector header.");
    code.open("pub fn take_vector_header(&mut self) -> ::rtd_bin::Result<usize> {");
    code.line("let count = self.reader.take_vector_header()?;");
    code.line("self.bump(count)?;");
    code.line("Ok(count)");
    code.close("}");
    code.line("");
    code.line("/// Reads and bounds a bare vector length.");
    code.open("pub fn take_bare_vector_len(&mut self) -> ::rtd_bin::Result<usize> {");
    code.line("let count = self.reader.take_i32()?;");
    code.line("if count < 0 {");
    code.line("    return Err(::rtd_bin::Error::NegativeVectorCount(count));");
    code.line("}");
    code.line("self.bump(count as usize)?;");
    code.line("Ok(count as usize)");
    code.close("}");
    code.line("");
    code.line("/// Enters one boxed nesting level.");
    code.open("pub fn enter(&mut self) -> ::rtd_bin::Result<()> {");
    code.line("self.depth += 1;");
    code.open("if self.depth > self.limits.max_depth {");
    code.line("return Err(::rtd_bin::Error::InvalidLength { length: self.depth, context: \"max depth\" });");
    code.close("}");
    code.line("Ok(())");
    code.close("}");
    code.line("");
    code.line("/// Leaves one boxed nesting level.");
    code.open("pub fn leave(&mut self) {");
    code.line("self.depth = self.depth.saturating_sub(1);");
    code.close("}");
    code.line("");
    code.open("fn bump(&mut self, count: usize) -> ::rtd_bin::Result<()> {");
    code.open("if count > self.limits.max_vector_elements {");
    code.line("return Err(::rtd_bin::Error::InvalidLength { length: count, context: \"vector elements\" });");
    code.close("}");
    code.line("self.aggregate = self.aggregate.saturating_add(count);");
    code.open("if self.aggregate > self.limits.max_aggregate_elements {");
    code.line("return Err(::rtd_bin::Error::InvalidLength { length: self.aggregate, context: \"aggregate elements\" });");
    code.close("}");
    code.line("Ok(())");
    code.close("}");
    code.close("}");
    code.line("");
}
