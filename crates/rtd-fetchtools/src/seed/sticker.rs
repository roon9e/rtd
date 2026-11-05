//! `stickerfetch` seed values.
//!
//! Mirrors the Go `cmd/stickerfetch` seed structs, which in turn align with the
//! parser in `telesrv`'s `internal/app/files/seed.go`. The JSON tags (including
//! `omitempty` and the `_` constructor field) are reproduced exactly so the
//! output is byte-identical.

use crate::gojson::Value;

/// A sticker-set reference embedded in an attribute.
#[derive(Debug, Clone, Default)]
pub struct InputSetRef {
    /// Set id.
    pub id: i64,
    /// Set access hash.
    pub access_hash: i64,
}

/// One document attribute.
#[derive(Debug, Clone, Default)]
pub struct Attribute {
    /// Constructor name (`DocumentAttribute…`).
    pub type_: String,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
    /// Sticker alt text.
    pub alt: String,
    /// Mask flag.
    pub mask: bool,
    /// Custom-emoji free flag.
    pub free: bool,
    /// Custom-emoji text-colour flag.
    pub text_color: bool,
    /// File name.
    pub file_name: String,
    /// Video duration.
    pub duration: f64,
    /// Round-message flag.
    pub round_message: bool,
    /// Streaming flag.
    pub supports_streaming: bool,
    /// Voice flag.
    pub voice: bool,
    /// Audio title.
    pub title: String,
    /// Audio performer.
    pub performer: String,
    /// Sticker set reference.
    pub stickerset: Option<InputSetRef>,
}

/// One thumbnail, inline or file-backed.
#[derive(Debug, Clone, Default)]
pub struct Thumb {
    /// Constructor name (`PhotoSize`, `PhotoPathSize`, …).
    pub type_: String,
    /// Size type (`m`, `j`, …).
    pub size_type: String,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
    /// Byte size.
    pub size: i32,
    /// Inline bytes, hex-encoded.
    pub bytes: String,
    /// Progressive sizes.
    pub sizes: Vec<i32>,
}

/// One exported document.
#[derive(Debug, Clone, Default)]
pub struct Document {
    /// Document id.
    pub id: i64,
    /// Access hash.
    pub access_hash: i64,
    /// File reference, hex-encoded.
    pub file_reference: String,
    /// Unix date rendered as RFC 3339.
    pub date: String,
    /// MIME type.
    pub mime_type: String,
    /// Byte size.
    pub size: i64,
    /// Datacenter id.
    pub dc_id: i32,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Thumbnails.
    pub thumbs: Vec<Thumb>,
}

/// One sticker pack.
#[derive(Debug, Clone, Default)]
pub struct Pack {
    /// Emoticon.
    pub emoticon: String,
    /// Document ids.
    pub documents: Vec<i64>,
}

/// The sticker set.
#[derive(Debug, Clone, Default)]
pub struct Set {
    /// Set id.
    pub id: i64,
    /// Access hash.
    pub access_hash: i64,
    /// Title.
    pub title: String,
    /// Short name.
    pub short_name: String,
    /// Document count.
    pub count: i32,
    /// Set hash.
    pub hash: i32,
    /// Official flag.
    pub official: bool,
    /// Masks flag.
    pub masks: bool,
    /// Emoji flag.
    pub emojis: bool,
    /// Inline set thumbnails.
    pub thumbs: Vec<Thumb>,
    /// Thumbnail datacenter id.
    pub thumb_dc_id: i32,
    /// Thumbnail version.
    pub thumb_version: i32,
    /// Thumbnail document id.
    pub thumb_document_id: Option<i64>,
}

/// A `set_info.json` root.
#[derive(Debug, Clone, Default)]
pub struct SetInfo {
    /// API call name.
    pub api_call: String,
    /// Go input-set type name.
    pub input_set_type: String,
    /// The set.
    pub set: Set,
    /// Packs.
    pub packs: Vec<Pack>,
    /// Documents.
    pub documents: Vec<Document>,
}

impl SetInfo {
    /// Renders the `set_info.json` value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("api_call", Value::Str(self.api_call.clone())),
            ("input_set_type", Value::Str(self.input_set_type.clone())),
            (
                "result",
                Value::object(vec![
                    ("_", Value::Str("StickerSet".to_owned())),
                    ("set", set_value(&self.set)),
                    (
                        "packs",
                        Value::Array(self.packs.iter().map(pack_value).collect()),
                    ),
                    (
                        "documents",
                        Value::Array(self.documents.iter().map(document_value).collect()),
                    ),
                ]),
            ),
        ])
    }
}

/// Renders one [`Set`], honouring `omitempty` on the optional fields.
#[must_use]
pub fn set_value(set: &Set) -> Value {
    let mut fields: Vec<(&str, Value)> = vec![
        ("id", Value::Int(set.id)),
        ("access_hash", Value::Int(set.access_hash)),
        ("title", Value::Str(set.title.clone())),
        ("short_name", Value::Str(set.short_name.clone())),
        ("count", Value::Int(i64::from(set.count))),
        ("hash", Value::Int(i64::from(set.hash))),
        ("official", Value::Bool(set.official)),
        ("masks", Value::Bool(set.masks)),
        ("emojis", Value::Bool(set.emojis)),
    ];
    if !set.thumbs.is_empty() {
        fields.push((
            "thumbs",
            Value::Array(set.thumbs.iter().map(thumb_value).collect()),
        ));
    }
    if set.thumb_dc_id != 0 {
        fields.push(("thumb_dc_id", Value::Int(i64::from(set.thumb_dc_id))));
    }
    if set.thumb_version != 0 {
        fields.push(("thumb_version", Value::Int(i64::from(set.thumb_version))));
    }
    if let Some(id) = set.thumb_document_id {
        fields.push(("thumb_document_id", Value::Int(id)));
    }
    Value::object(fields)
}

/// Renders one [`Pack`].
#[must_use]
pub fn pack_value(pack: &Pack) -> Value {
    Value::object(vec![
        ("emoticon", Value::Str(pack.emoticon.clone())),
        (
            "documents",
            Value::Array(pack.documents.iter().map(|id| Value::Int(*id)).collect()),
        ),
    ])
}

/// Renders one [`Document`] (`omitempty` on `thumbs`).
#[must_use]
pub fn document_value(document: &Document) -> Value {
    let mut fields: Vec<(&str, Value)> = vec![
        ("id", Value::Int(document.id)),
        ("access_hash", Value::Int(document.access_hash)),
        (
            "file_reference",
            Value::Str(document.file_reference.clone()),
        ),
        ("date", Value::Str(document.date.clone())),
        ("mime_type", Value::Str(document.mime_type.clone())),
        ("size", Value::Int(document.size)),
        ("dc_id", Value::Int(i64::from(document.dc_id))),
        (
            "attributes",
            Value::Array(document.attributes.iter().map(attribute_value).collect()),
        ),
    ];
    if !document.thumbs.is_empty() {
        fields.push((
            "thumbs",
            Value::Array(document.thumbs.iter().map(thumb_value).collect()),
        ));
    }
    Value::object(fields)
}

/// Renders one [`Attribute`].
#[must_use]
pub fn attribute_value(attribute: &Attribute) -> Value {
    let mut fields: Vec<(&str, Value)> = vec![("_", Value::Str(attribute.type_.clone()))];
    if attribute.w != 0 {
        fields.push(("w", Value::Int(i64::from(attribute.w))));
    }
    if attribute.h != 0 {
        fields.push(("h", Value::Int(i64::from(attribute.h))));
    }
    if !attribute.alt.is_empty() {
        fields.push(("alt", Value::Str(attribute.alt.clone())));
    }
    if attribute.mask {
        fields.push(("mask", Value::Bool(true)));
    }
    if attribute.free {
        fields.push(("free", Value::Bool(true)));
    }
    if attribute.text_color {
        fields.push(("text_color", Value::Bool(true)));
    }
    if !attribute.file_name.is_empty() {
        fields.push(("file_name", Value::Str(attribute.file_name.clone())));
    }
    if attribute.duration != 0.0 {
        fields.push(("duration", Value::Float(attribute.duration)));
    }
    if attribute.round_message {
        fields.push(("round_message", Value::Bool(true)));
    }
    if attribute.supports_streaming {
        fields.push(("supports_streaming", Value::Bool(true)));
    }
    if attribute.voice {
        fields.push(("voice", Value::Bool(true)));
    }
    if !attribute.title.is_empty() {
        fields.push(("title", Value::Str(attribute.title.clone())));
    }
    if !attribute.performer.is_empty() {
        fields.push(("performer", Value::Str(attribute.performer.clone())));
    }
    if let Some(stickerset) = &attribute.stickerset {
        fields.push((
            "stickerset",
            Value::object(vec![
                ("id", Value::Int(stickerset.id)),
                ("access_hash", Value::Int(stickerset.access_hash)),
            ]),
        ));
    }
    Value::object(fields)
}

/// Renders one [`Thumb`].
#[must_use]
pub fn thumb_value(thumb: &Thumb) -> Value {
    let mut fields: Vec<(&str, Value)> = vec![
        ("_", Value::Str(thumb.type_.clone())),
        ("type", Value::Str(thumb.size_type.clone())),
    ];
    if thumb.w != 0 {
        fields.push(("w", Value::Int(i64::from(thumb.w))));
    }
    if thumb.h != 0 {
        fields.push(("h", Value::Int(i64::from(thumb.h))));
    }
    if thumb.size != 0 {
        fields.push(("size", Value::Int(i64::from(thumb.size))));
    }
    if !thumb.bytes.is_empty() {
        fields.push(("bytes", Value::Str(thumb.bytes.clone())));
    }
    if !thumb.sizes.is_empty() {
        fields.push(("sizes", Value::from(thumb.sizes.clone())));
    }
    Value::object(fields)
}

/// One available message effect.
#[derive(Debug, Clone, Default)]
pub struct Effect {
    /// Effect id.
    pub id: i64,
    /// Emoticon.
    pub emoticon: String,
    /// Static icon document id.
    pub static_icon_id: Option<i64>,
    /// Effect sticker document id.
    pub effect_sticker_id: i64,
    /// Effect animation document id.
    pub effect_animation_id: Option<i64>,
    /// Premium flag.
    pub premium_required: bool,
}

/// The `effects.json` root.
#[derive(Debug, Clone, Default)]
pub struct EffectsFile {
    /// API call name.
    pub api_call: String,
    /// Effects.
    pub effects: Vec<Effect>,
    /// Documents.
    pub documents: Vec<Document>,
}

impl EffectsFile {
    /// Renders the `effects.json` value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("api_call", Value::Str(self.api_call.clone())),
            (
                "result",
                Value::object(vec![
                    (
                        "effects",
                        array_or_null(self.effects.iter().map(effect_value).collect()),
                    ),
                    (
                        "documents",
                        array_or_null(self.documents.iter().map(document_value).collect()),
                    ),
                ]),
            ),
        ])
    }
}

/// Renders one [`Effect`].
#[must_use]
pub fn effect_value(effect: &Effect) -> Value {
    let mut fields: Vec<(&str, Value)> = vec![
        ("id", Value::Int(effect.id)),
        ("emoticon", Value::Str(effect.emoticon.clone())),
    ];
    if let Some(id) = effect.static_icon_id {
        fields.push(("static_icon_id", Value::Int(id)));
    }
    fields.push(("effect_sticker_id", Value::Int(effect.effect_sticker_id)));
    if let Some(id) = effect.effect_animation_id {
        fields.push(("effect_animation_id", Value::Int(id)));
    }
    if effect.premium_required {
        fields.push(("premium_required", Value::Bool(true)));
    }
    Value::object(fields)
}

/// One available reaction.
#[derive(Debug, Clone, Default)]
pub struct Reaction {
    /// Reaction emoticon.
    pub reaction: String,
    /// Title.
    pub title: String,
    /// Inactive flag.
    pub inactive: bool,
    /// Premium flag.
    pub premium: bool,
    /// Static icon.
    pub static_icon: Option<Document>,
    /// Appear animation.
    pub appear_animation: Option<Document>,
    /// Select animation.
    pub select_animation: Option<Document>,
    /// Activate animation.
    pub activate_animation: Option<Document>,
    /// Effect animation.
    pub effect_animation: Option<Document>,
    /// Around animation.
    pub around_animation: Option<Document>,
    /// Center icon.
    pub center_icon: Option<Document>,
}

/// Renders the `available_reactions_raw.json` value.
#[must_use]
pub fn reactions_value(reactions: &[Reaction]) -> Value {
    Value::object(vec![(
        "result",
        Value::object(vec![(
            "reactions",
            Value::Array(reactions.iter().map(reaction_value).collect()),
        )]),
    )])
}

/// Renders one [`Reaction`].
#[must_use]
pub fn reaction_value(reaction: &Reaction) -> Value {
    let mut fields: Vec<(&str, Value)> = vec![
        ("reaction", Value::Str(reaction.reaction.clone())),
        ("title", Value::Str(reaction.title.clone())),
    ];
    if reaction.inactive {
        fields.push(("inactive", Value::Bool(true)));
    }
    if reaction.premium {
        fields.push(("premium", Value::Bool(true)));
    }
    push_document(&mut fields, "static_icon", reaction.static_icon.as_ref());
    push_document(
        &mut fields,
        "appear_animation",
        reaction.appear_animation.as_ref(),
    );
    push_document(
        &mut fields,
        "select_animation",
        reaction.select_animation.as_ref(),
    );
    push_document(
        &mut fields,
        "activate_animation",
        reaction.activate_animation.as_ref(),
    );
    push_document(
        &mut fields,
        "effect_animation",
        reaction.effect_animation.as_ref(),
    );
    push_document(
        &mut fields,
        "around_animation",
        reaction.around_animation.as_ref(),
    );
    push_document(&mut fields, "center_icon", reaction.center_icon.as_ref());
    Value::object(fields)
}

/// Pushes a document field when present (`omitempty`).
fn push_document(
    fields: &mut Vec<(&'static str, Value)>,
    key: &'static str,
    document: Option<&Document>,
) {
    if let Some(document) = document {
        fields.push((key, document_value(document)));
    }
}

/// Renders an append-built slice: `null` when empty.
fn array_or_null(values: Vec<Value>) -> Value {
    if values.is_empty() {
        Value::Null
    } else {
        Value::Array(values)
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, Reaction, reactions_value};

    /// Locks the reaction export layout against the Go golden fixture
    /// (`cmd/stickerfetch/testdata/reactions_export`).
    #[test]
    fn renders_reaction_document() {
        let document = Document {
            id: 1_000_001,
            access_hash: 10_000_010,
            file_reference: "0102".to_owned(),
            date: "2023-11-14T22:13:20Z".to_owned(),
            mime_type: "image/webp".to_owned(),
            size: 16,
            dc_id: 2,
            attributes: vec![super::Attribute {
                type_: "DocumentAttributeImageSize".to_owned(),
                w: 512,
                h: 512,
                ..super::Attribute::default()
            }],
            thumbs: vec![
                super::Thumb {
                    type_: "PhotoPathSize".to_owned(),
                    size_type: "j".to_owned(),
                    bytes: "1020".to_owned(),
                    ..super::Thumb::default()
                },
                super::Thumb {
                    type_: "PhotoSize".to_owned(),
                    size_type: "m".to_owned(),
                    w: 128,
                    h: 128,
                    size: 15,
                    ..super::Thumb::default()
                },
            ],
        };
        let reaction = Reaction {
            reaction: "👍".to_owned(),
            title: "Thumbs Up".to_owned(),
            static_icon: Some(document),
            ..Reaction::default()
        };
        let rendered = reactions_value(&[reaction]).render("  ");
        assert!(rendered.contains("\"static_icon\": {"));
        assert!(rendered.contains("\"_\": \"DocumentAttributeImageSize\""));
        assert!(rendered.contains("\"bytes\": \"1020\""));
        assert!(rendered.contains("\"date\": \"2023-11-14T22:13:20Z\""));
    }
}
