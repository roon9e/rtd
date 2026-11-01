//! A line-oriented TL schema parser.
//!
//! The grammar and the wire-identifier rules mirror `github.com/gotd/tl`, which
//! the Go reference generator consumes. Layer schema files must carry an
//! explicit `#crc32` identifier on every constructor and method line; the
//! single non-layered MTProto schema (`mt.tl`) has one definition without an
//! identifier, so callers opt into computing the standard TL CRC32 there.

/// Which half of a schema a definition belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Category {
    /// A constructor (`---types---`, the default).
    Type,
    /// An RPC method (`---functions---`).
    Function,
}

impl Category {
    /// The lowercase category name used in semantic keys and policy keys.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Category::Type => "type",
            Category::Function => "function",
        }
    }
}

/// A parsed type reference, retaining all wire-relevant detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawType {
    /// Leading namespace components, e.g. `["auth"]` for `auth.SentCode`.
    pub(crate) namespace: Vec<String>,
    /// The final name component.
    pub(crate) name: String,
    /// Whether the type is bare (no constructor identifier written).
    pub(crate) bare: bool,
    /// Whether the type used the `%` bare marker.
    pub(crate) percent: bool,
    /// Whether the type is a generic reference such as `!X`.
    pub(crate) generic_ref: bool,
    /// The generic argument for `Vector<T>` / `X<Y>`.
    pub(crate) arg: Option<Box<RawType>>,
}

impl RawType {
    /// Returns the dotted qualified name.
    pub(crate) fn qualified(&self) -> String {
        if self.namespace.is_empty() {
            self.name.clone()
        } else {
            format!("{}.{}", self.namespace.join("."), self.name)
        }
    }

    /// Parses a single type reference.
    ///
    /// # Errors
    ///
    /// Returns a message when the reference is malformed.
    pub(crate) fn parse(mut text: &str) -> Result<Self, String> {
        let mut bare = false;
        let mut percent = false;
        let mut generic_ref = false;
        match text.chars().next() {
            None => return Err("empty type".to_owned()),
            Some('%') => {
                bare = true;
                percent = true;
                text = &text[1..];
            }
            Some('!') => {
                generic_ref = true;
                text = &text[1..];
            }
            _ => {}
        }
        let mut arg = None;
        if let Some(open) = text.find('<') {
            if !text.ends_with('>') {
                return Err(format!("invalid generic in {text:?}"));
            }
            arg = Some(Box::new(RawType::parse(&text[open + 1..text.len() - 1])?));
            text = &text[..open];
        }
        if text.starts_with('.') {
            return Err("type cannot start with '.'".to_owned());
        }
        let components: Vec<&str> = text.split('.').collect();
        let (namespace, name) = match components.split_last() {
            Some((name, namespace)) => (
                namespace
                    .iter()
                    .map(|part| (*part).to_owned())
                    .collect::<Vec<_>>(),
                (*name).to_owned(),
            ),
            None => return Err("blank type".to_owned()),
        };
        if name.is_empty() {
            return Err("blank type name".to_owned());
        }
        if !is_valid_identifier(&name) {
            return Err(format!("invalid type name {name:?}"));
        }
        for part in &namespace {
            if !is_valid_identifier(part) {
                return Err(format!("invalid namespace part {part:?}"));
            }
        }
        if !percent
            && let Some(first) = name.chars().next()
            && first.is_ascii_lowercase()
        {
            bare = true;
        }
        Ok(RawType {
            namespace,
            name,
            bare,
            percent,
            generic_ref,
            arg,
        })
    }
}

/// A conditional flag such as `flags.0?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawFlag {
    /// The flags word this bit belongs to (`flags`, `flags2`, ...).
    pub(crate) word: String,
    /// The bit index (`0`..=`31`).
    pub(crate) bit: u8,
}

/// A parsed method or constructor parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawParam {
    /// The parameter name, or empty when anonymous.
    pub(crate) name: String,
    /// The parameter type.
    pub(crate) ty: RawType,
    /// Whether the parameter is an inline flags word (`#`).
    pub(crate) flags: bool,
    /// The presence condition, if the parameter is conditional.
    pub(crate) flag: Option<RawFlag>,
    /// Whether the parameter is a `{X:Type}` generic declaration.
    pub(crate) type_definition: bool,
}

impl RawParam {
    fn parse(text: &str) -> Result<Self, String> {
        if let Some(rest) = text.strip_prefix('{') {
            let inner = rest
                .strip_suffix(":Type}")
                .ok_or_else(|| format!("unexpected generic parameter {text:?}"))?;
            let name = inner.split(':').next().unwrap_or_default().to_owned();
            return Ok(RawParam {
                name,
                ty: RawType {
                    namespace: Vec::new(),
                    name: String::new(),
                    bare: false,
                    percent: false,
                    generic_ref: false,
                    arg: None,
                },
                flags: false,
                flag: None,
                type_definition: true,
            });
        }
        let (name, mut rest) = match text.split_once(':') {
            Some((name, rest)) => (name.to_owned(), rest),
            None => (String::new(), text),
        };
        if rest == "#" {
            return Ok(RawParam {
                name,
                ty: RawType {
                    namespace: Vec::new(),
                    name: String::new(),
                    bare: false,
                    percent: false,
                    generic_ref: false,
                    arg: None,
                },
                flags: true,
                flag: None,
                type_definition: false,
            });
        }
        let mut flag = None;
        if let Some(question) = rest.find('?') {
            let (word, bit) = rest[..question]
                .split_once('.')
                .ok_or_else(|| format!("invalid flag condition {:?}", &rest[..question]))?;
            let bit = bit
                .parse::<u8>()
                .map_err(|_| format!("invalid flag bit in {:?}", &rest[..question]))?;
            flag = Some(RawFlag {
                word: word.to_owned(),
                bit,
            });
            rest = &rest[question + 1..];
        }
        Ok(RawParam {
            name,
            ty: RawType::parse(rest)?,
            flags: false,
            flag,
            type_definition: false,
        })
    }
}

/// A parsed definition (constructor or method).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawDefinition {
    /// Constructor or method category.
    pub(crate) category: Category,
    /// Leading namespace components.
    pub(crate) namespace: Vec<String>,
    /// The final name component.
    pub(crate) name: String,
    /// The explicit wire identifier.
    pub(crate) id: u32,
    /// Whether the wire identifier was explicit in the source.
    pub(crate) explicit_id: bool,
    /// Ordered, non-generic parameters.
    pub(crate) params: Vec<RawParam>,
    /// The generic parameter names declared with `{X:Type}`.
    pub(crate) generic_params: Vec<String>,
    /// The result type.
    pub(crate) result: RawType,
    /// Whether the definition is marked bare with a trailing `?`.
    pub(crate) base: bool,
}

impl RawDefinition {
    /// Returns the dotted qualified name.
    pub(crate) fn qualified(&self) -> String {
        if self.namespace.is_empty() {
            self.name.clone()
        } else {
            format!("{}.{}", self.namespace.join("."), self.name)
        }
    }
}

/// A parsed schema file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawSchema {
    /// The `// LAYER N` value, or `0` when absent.
    pub(crate) layer: i32,
    /// Definitions in source order.
    pub(crate) definitions: Vec<RawDefinition>,
}

/// Returns `true` when the text is a valid TL identifier.
pub(crate) fn is_valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parses a schema, requiring an explicit identifier on every definition line.
///
/// # Errors
///
/// Returns a message identifying the offending line when a declaration omits
/// its `#crc32`, or when the grammar is not understood.
pub(crate) fn parse_schema(input: &str) -> Result<RawSchema, String> {
    parse_internal(input, true)
}

/// Parses a schema, computing the standard TL CRC32 for definitions without an
/// explicit identifier.
///
/// # Errors
///
/// Returns a message when the grammar is not understood.
pub(crate) fn parse_schema_with_crc(input: &str) -> Result<RawSchema, String> {
    parse_internal(input, false)
}

fn parse_internal(input: &str, require_explicit: bool) -> Result<RawSchema, String> {
    let mut schema = RawSchema {
        layer: 0,
        definitions: Vec::new(),
    };
    let mut category = Category::Type;
    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = raw_line.trim();
        match trimmed {
            "" => continue,
            "---functions---" => {
                category = Category::Function;
                continue;
            }
            "---types---" => {
                category = Category::Type;
                continue;
            }
            _ => {}
        }
        if let Some(layer) = trimmed.strip_prefix("// LAYER ") {
            schema.layer = layer
                .trim()
                .parse()
                .map_err(|_| format!("line {line_number}: invalid layer annotation"))?;
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("vector") && trimmed.ends_with("= Vector t;") {
            // The built-in generic vector definition is handled structurally.
            continue;
        }
        schema.definitions.push(parse_definition(
            trimmed,
            category,
            line_number,
            require_explicit,
        )?);
    }
    Ok(schema)
}

fn parse_definition(
    line: &str,
    category: Category,
    line_number: usize,
    require_explicit: bool,
) -> Result<RawDefinition, String> {
    let without_semicolon = line.strip_suffix(';').unwrap_or(line);
    let (left, right) = without_semicolon
        .split_once('=')
        .ok_or_else(|| format!("line {line_number}: definition has no '='"))?;
    if right.contains('=') {
        return Err(format!("line {line_number}: definition has multiple '='"));
    }
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() || right.is_empty() {
        return Err(format!("line {line_number}: definition part is blank"));
    }
    let mut parts = left.split(' ');
    let head = parts
        .next()
        .ok_or_else(|| format!("line {line_number}: missing definition name"))?;
    let (name_token, id_token) = match head.split_once('#') {
        Some((name, id)) => (name, Some(id)),
        None => (head, None),
    };
    if name_token.is_empty() {
        return Err(format!("line {line_number}: blank definition name"));
    }
    let (id, explicit_id) = match id_token {
        Some(text) if !text.is_empty() => {
            let id = u32::from_str_radix(text, 16)
                .map_err(|_| format!("line {line_number}: invalid identifier {text:?}"))?;
            (id, true)
        }
        _ => {
            if require_explicit {
                return Err(format!(
                    "line {line_number}: definition {name_token:?} has no explicit wire identifier"
                ));
            }
            (crate::parse::crc32(without_semicolon.as_bytes()), false)
        }
    };
    let (namespace, name) = split_name(name_token);
    if !is_valid_identifier(&name) {
        return Err(format!("line {line_number}: invalid name {name:?}"));
    }
    for component in &namespace {
        if !is_valid_identifier(component) {
            return Err(format!(
                "line {line_number}: invalid namespace {component:?}"
            ));
        }
    }

    let result = RawType::parse(right)
        .map_err(|error| format!("line {line_number}: result type: {error}"))?;

    let mut params = Vec::new();
    let mut generic_params = Vec::new();
    let mut base = false;
    for token in parts {
        if token.is_empty() {
            return Err(format!("line {line_number}: empty parameter"));
        }
        if token == "?" {
            base = true;
            continue;
        }
        let param = RawParam::parse(token)
            .map_err(|error| format!("line {line_number}: parameter {token:?}: {error}"))?;
        if param.type_definition {
            generic_params.push(param.name.clone());
            continue;
        }
        if param.ty.generic_ref && !generic_params.contains(&param.ty.name) {
            return Err(format!(
                "line {line_number}: undefined generic parameter {}",
                param.ty.name
            ));
        }
        params.push(param);
    }

    Ok(RawDefinition {
        category,
        namespace,
        name,
        id,
        explicit_id,
        params,
        generic_params,
        result,
        base,
    })
}

fn split_name(name: &str) -> (Vec<String>, String) {
    match name.rsplit_once('.') {
        Some((namespace, local)) => (
            namespace.split('.').map(str::to_owned).collect(),
            local.to_owned(),
        ),
        None => (Vec::new(), name.to_owned()),
    }
}

/// The standard IEEE CRC32 used for implicit TL constructor identifiers.
///
/// This is a local implementation so the parser stays dependency-free.
pub(crate) fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::{Category, crc32, parse_schema, parse_schema_with_crc};

    #[test]
    fn parses_layer_and_definition() {
        let schema = parse_schema("// LAYER 232\n\nfoo#12345678 a:int b:string = Foo;\n").unwrap();
        assert_eq!(schema.layer, 232);
        assert_eq!(schema.definitions.len(), 1);
        let def = &schema.definitions[0];
        assert_eq!(def.category, Category::Type);
        assert_eq!(def.name, "foo");
        assert_eq!(def.id, 0x1234_5678);
        assert!(def.explicit_id);
        assert_eq!(def.params.len(), 2);
    }

    #[test]
    fn rejects_missing_explicit_identifier() {
        let error = parse_schema("foo a:int = Foo;\n").unwrap_err();
        assert!(error.contains("no explicit wire identifier"), "{error}");
    }

    #[test]
    fn computes_crc_for_mt() {
        let schema = parse_schema_with_crc("message msg_id:long = Message;\n").unwrap();
        assert!(!schema.definitions[0].explicit_id);
        assert_eq!(
            schema.definitions[0].id,
            crc32(b"message msg_id:long = Message")
        );
    }

    #[test]
    fn parses_flags_and_generics() {
        let schema =
            parse_schema("invokeWithLayer#da9b0d0d {X:Type} layer:int query:!X = X;\n").unwrap();
        let def = &schema.definitions[0];
        assert_eq!(def.category, Category::Type);
        assert_eq!(def.generic_params, vec!["X".to_owned()]);
        assert_eq!(def.params.len(), 2);
        assert!(def.params[1].ty.generic_ref);
    }

    #[test]
    fn parses_flag_conditions() {
        let schema = parse_schema("x#1 flags:# a:flags.3?int b:flags.0?true = X;\n").unwrap();
        let def = &schema.definitions[0];
        assert!(def.params[0].flags);
        assert_eq!(def.params[1].flag.as_ref().unwrap().bit, 3);
        assert_eq!(def.params[1].flag.as_ref().unwrap().word, "flags");
    }
}
