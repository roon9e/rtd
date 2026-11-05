//! `langpackfetch` seed values: `.strings` files and the pack manifest.

use rtd_tg::{LangPackDifference, LangPackString};

use crate::gojson::Value;
use crate::goquote::quote;

/// Converts a langpack difference into the telesrv `.strings` body.
///
/// Mirrors the Go writer: `"key" = "value";` per line, plural values as
/// `key#zero`, `key#one`, … with empty forms skipped, and deleted entries
/// dropped. Returns the text and the number of counted strings.
#[must_use]
pub fn strings_body(difference: &LangPackDifference) -> (String, usize) {
    let mut out = String::new();
    let mut count = 0usize;
    for string in &difference.strings {
        match string {
            LangPackString::LangPackString { key, value } => {
                out.push_str(&quote(key));
                out.push_str(" = ");
                out.push_str(&quote(value));
                out.push_str(";\n");
                count += 1;
            }
            LangPackString::LangPackStringPluralized {
                key,
                zero_value,
                one_value,
                two_value,
                few_value,
                many_value,
                other_value,
                ..
            } => {
                emit_plural(&mut out, key, "#zero", zero_value.as_deref());
                emit_plural(&mut out, key, "#one", one_value.as_deref());
                emit_plural(&mut out, key, "#two", two_value.as_deref());
                emit_plural(&mut out, key, "#few", few_value.as_deref());
                emit_plural(&mut out, key, "#many", many_value.as_deref());
                emit_plural(&mut out, key, "#other", Some(other_value));
                count += 1;
            }
            LangPackString::LangPackStringDeleted { .. } => {}
        }
    }
    (out, count)
}

/// Appends one plural form when its value is non-empty.
fn emit_plural(out: &mut String, key: &str, suffix: &str, value: Option<&str>) {
    if let Some(value) = value
        && !value.is_empty()
    {
        out.push_str(&quote(&format!("{key}{suffix}")));
        out.push_str(" = ");
        out.push_str(&quote(value));
        out.push_str(";\n");
    }
}

/// One language entry of `official-language-packs.json`.
#[derive(Debug, Clone)]
pub struct LanguageManifest {
    /// Lowercased language code.
    pub lang_code: String,
    /// English name.
    pub name: String,
    /// Native name.
    pub native_name: String,
    /// Base language code, when the pack inherits one.
    pub base_lang_code: Option<String>,
    /// Plural rule code.
    pub plural_code: String,
    /// Whether the language is official.
    pub official: bool,
    /// Whether the language is right-to-left.
    pub rtl: bool,
    /// Whether the language is in beta.
    pub beta: bool,
    /// Total string count.
    pub strings_count: i32,
    /// Translated string count.
    pub translated_count: i32,
    /// Translation platform URL.
    pub translations_url: String,
    /// Pack version.
    pub version: i32,
    /// Relative path of the `.strings` file.
    pub file: String,
    /// SHA-256 of the `.strings` file.
    pub sha256: String,
}

impl LanguageManifest {
    /// Renders the manifest entry with Go `omitempty` semantics.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut fields: Vec<(&str, Value)> = vec![
            ("lang_code", Value::Str(self.lang_code.clone())),
            ("name", Value::Str(self.name.clone())),
            ("native_name", Value::Str(self.native_name.clone())),
        ];
        if let Some(base) = &self.base_lang_code
            && !base.is_empty()
        {
            fields.push(("base_lang_code", Value::Str(base.clone())));
        }
        fields.push(("plural_code", Value::Str(self.plural_code.clone())));
        fields.push(("official", Value::Bool(self.official)));
        if self.rtl {
            fields.push(("rtl", Value::Bool(true)));
        }
        if self.beta {
            fields.push(("beta", Value::Bool(true)));
        }
        fields.push(("strings_count", Value::Int(i64::from(self.strings_count))));
        fields.push((
            "translated_count",
            Value::Int(i64::from(self.translated_count)),
        ));
        if !self.translations_url.is_empty() {
            fields.push((
                "translations_url",
                Value::Str(self.translations_url.clone()),
            ));
        }
        fields.push(("version", Value::Int(i64::from(self.version))));
        fields.push(("file", Value::Str(self.file.clone())));
        fields.push(("sha256", Value::Str(self.sha256.clone())));
        Value::object(fields)
    }
}

/// One pack entry of `official-language-packs.json`.
#[derive(Debug, Clone)]
pub struct PackManifest {
    /// Pack name.
    pub name: String,
    /// Languages in the pack.
    pub languages: Vec<LanguageManifest>,
}

impl PackManifest {
    /// Renders the pack entry.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let languages: Vec<Value> = self
            .languages
            .iter()
            .map(LanguageManifest::to_value)
            .collect();
        Value::object(vec![
            ("name", Value::Str(self.name.clone())),
            ("languages", array_or_null(languages)),
        ])
    }
}

/// The `official-language-packs.json` root.
#[derive(Debug, Clone)]
pub struct Manifest {
    /// Languages in the pack.
    pub packs: Vec<PackManifest>,
}

impl Manifest {
    /// Creates an empty manifest with schema 1.
    #[must_use]
    pub fn new() -> Self {
        Self { packs: Vec::new() }
    }

    /// Renders the manifest.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let packs: Vec<Value> = self.packs.iter().map(PackManifest::to_value).collect();
        Value::object(vec![
            ("schema", Value::Int(1)),
            ("packs", array_or_null(packs)),
        ])
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders an append-built slice: `null` when empty (Go marshals nil as null).
fn array_or_null(values: Vec<Value>) -> Value {
    if values.is_empty() {
        Value::Null
    } else {
        Value::Array(values)
    }
}
