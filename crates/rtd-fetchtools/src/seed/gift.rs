//! `giftfetch` seed values: the star-gift catalog manifest.
//!
//! Mirrors the Go `cmd/giftfetch` manifest structs, including their `omitempty`
//! and null-vs-[] slice behaviour, so `manifest.json` is byte-identical.

use crate::gojson::Value;

/// One written artifact.
#[derive(Debug, Clone, Default)]
pub struct FileArtifact {
    /// Artifact kind (`document`, `photo`, `video`, `tl`).
    pub kind: String,
    /// Artifact subtype (thumbnail type).
    pub type_: String,
    /// Relative path.
    pub path: String,
    /// Byte size.
    pub size: i64,
    /// SHA-256.
    pub sha256: String,
}

impl FileArtifact {
    /// Renders the artifact.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut fields: Vec<(&str, Value)> = Vec::new();
        if !self.kind.is_empty() {
            fields.push(("kind", Value::Str(self.kind.clone())));
        }
        if !self.type_.is_empty() {
            fields.push(("type", Value::Str(self.type_.clone())));
        }
        fields.push(("path", Value::Str(self.path.clone())));
        fields.push(("size", Value::Int(self.size)));
        fields.push(("sha256", Value::Str(self.sha256.clone())));
        Value::object(fields)
    }
}

/// A gift card background.
#[derive(Debug, Clone, Default)]
pub struct Background {
    /// Centre colour.
    pub center_color: i32,
    /// Edge colour.
    pub edge_color: i32,
    /// Text colour.
    pub text_color: i32,
}

/// One catalog gift.
#[derive(Debug, Clone, Default)]
pub struct GiftManifest {
    /// Catalog index.
    pub index: i32,
    /// Kind (`regular` or `unique`).
    pub kind: String,
    /// Gift id.
    pub id: i64,
    /// Base gift id (unique gifts).
    pub gift_id: i64,
    /// Title.
    pub title: String,
    /// Slug.
    pub slug: String,
    /// Unique number.
    pub number: i32,
    /// Price in stars.
    pub stars: i64,
    /// Convert-to-stars value.
    pub convert_stars: i64,
    /// Upgrade price in stars.
    pub upgrade_stars: i64,
    /// Minimum resale price.
    pub resell_min_stars: i64,
    /// Limited flag.
    pub limited: bool,
    /// Sold-out flag.
    pub sold_out: bool,
    /// Birthday flag.
    pub birthday: bool,
    /// Premium-required flag.
    pub require_premium: bool,
    /// Limited-per-user flag.
    pub limited_per_user: bool,
    /// Peer-colour-available flag.
    pub peer_color_available: bool,
    /// Auction flag.
    pub auction: bool,
    /// Remaining availability.
    pub availability_remains: i32,
    /// Total availability.
    pub availability_total: i32,
    /// Resale availability.
    pub availability_resale: i64,
    /// Issued availability.
    pub availability_issued: i32,
    /// Per-user total.
    pub per_user_total: i32,
    /// Per-user remaining.
    pub per_user_remains: i32,
    /// First sale date.
    pub first_sale_date: i32,
    /// Last sale date.
    pub last_sale_date: i32,
    /// Locked-until date.
    pub locked_until_date: i32,
    /// Auction slug.
    pub auction_slug: String,
    /// Gifts per round.
    pub gifts_per_round: i32,
    /// Auction start date.
    pub auction_start_date: i32,
    /// Upgrade variants.
    pub upgrade_variants: i32,
    /// Document ids.
    pub document_ids: Vec<i64>,
    /// Background.
    pub background: Option<Background>,
}

impl GiftManifest {
    /// Renders the gift entry.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut fields: Vec<(&str, Value)> = vec![
            ("index", Value::Int(i64::from(self.index))),
            ("kind", Value::Str(self.kind.clone())),
            ("id", Value::Int(self.id)),
        ];
        push_int_opt(&mut fields, "gift_id", self.gift_id);
        push_str_opt(&mut fields, "title", &self.title);
        push_str_opt(&mut fields, "slug", &self.slug);
        push_int_opt(&mut fields, "number", i64::from(self.number));
        push_int_opt(&mut fields, "stars", self.stars);
        push_int_opt(&mut fields, "convert_stars", self.convert_stars);
        push_int_opt(&mut fields, "upgrade_stars", self.upgrade_stars);
        push_int_opt(&mut fields, "resell_min_stars", self.resell_min_stars);
        push_bool_opt(&mut fields, "limited", self.limited);
        push_bool_opt(&mut fields, "sold_out", self.sold_out);
        push_bool_opt(&mut fields, "birthday", self.birthday);
        push_bool_opt(&mut fields, "require_premium", self.require_premium);
        push_bool_opt(&mut fields, "limited_per_user", self.limited_per_user);
        push_bool_opt(
            &mut fields,
            "peer_color_available",
            self.peer_color_available,
        );
        push_bool_opt(&mut fields, "auction", self.auction);
        push_int_opt(
            &mut fields,
            "availability_remains",
            i64::from(self.availability_remains),
        );
        push_int_opt(
            &mut fields,
            "availability_total",
            i64::from(self.availability_total),
        );
        push_int_opt(&mut fields, "availability_resale", self.availability_resale);
        push_int_opt(
            &mut fields,
            "availability_issued",
            i64::from(self.availability_issued),
        );
        push_int_opt(
            &mut fields,
            "per_user_total",
            i64::from(self.per_user_total),
        );
        push_int_opt(
            &mut fields,
            "per_user_remains",
            i64::from(self.per_user_remains),
        );
        push_int_opt(
            &mut fields,
            "first_sale_date",
            i64::from(self.first_sale_date),
        );
        push_int_opt(
            &mut fields,
            "last_sale_date",
            i64::from(self.last_sale_date),
        );
        push_int_opt(
            &mut fields,
            "locked_until_date",
            i64::from(self.locked_until_date),
        );
        push_str_opt(&mut fields, "auction_slug", &self.auction_slug);
        push_int_opt(
            &mut fields,
            "gifts_per_round",
            i64::from(self.gifts_per_round),
        );
        push_int_opt(
            &mut fields,
            "auction_start_date",
            i64::from(self.auction_start_date),
        );
        push_int_opt(
            &mut fields,
            "upgrade_variants",
            i64::from(self.upgrade_variants),
        );
        if !self.document_ids.is_empty() {
            fields.push((
                "document_ids",
                Value::Array(self.document_ids.iter().map(|id| Value::Int(*id)).collect()),
            ));
        }
        if let Some(background) = &self.background {
            fields.push((
                "background",
                Value::object(vec![
                    (
                        "center_color",
                        Value::Int(i64::from(background.center_color)),
                    ),
                    ("edge_color", Value::Int(i64::from(background.edge_color))),
                    ("text_color", Value::Int(i64::from(background.text_color))),
                ]),
            ));
        }
        Value::object(fields)
    }
}

/// A rarity descriptor.
#[derive(Debug, Clone, Default)]
pub struct Rarity {
    /// Kind.
    pub kind: String,
    /// Constructor id.
    pub constructor_id: String,
    /// Permille (only for `permille`).
    pub permille: Option<i32>,
}

impl Rarity {
    /// Renders the rarity.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut fields: Vec<(&str, Value)> = vec![
            ("kind", Value::Str(self.kind.clone())),
            ("constructor_id", Value::Str(self.constructor_id.clone())),
        ];
        if let Some(permille) = self.permille {
            fields.push(("permille", Value::Int(i64::from(permille))));
        }
        Value::object(fields)
    }
}

/// An upgrade model.
#[derive(Debug, Clone, Default)]
pub struct UpgradeModel {
    /// Name.
    pub name: String,
    /// Document id.
    pub document_id: i64,
    /// Crafted flag.
    pub crafted: bool,
    /// Rarity.
    pub rarity: Rarity,
}

/// An upgrade pattern.
#[derive(Debug, Clone, Default)]
pub struct UpgradePattern {
    /// Name.
    pub name: String,
    /// Document id.
    pub document_id: i64,
    /// Rarity.
    pub rarity: Rarity,
}

/// An upgrade backdrop.
#[derive(Debug, Clone, Default)]
pub struct UpgradeBackdrop {
    /// Name.
    pub name: String,
    /// Backdrop id.
    pub backdrop_id: i32,
    /// Centre colour.
    pub center_color: i32,
    /// Edge colour.
    pub edge_color: i32,
    /// Pattern colour.
    pub pattern_color: i32,
    /// Text colour.
    pub text_color: i32,
    /// Rarity.
    pub rarity: Rarity,
}

/// One gift's upgrade-attribute set.
#[derive(Debug, Clone, Default)]
pub struct UpgradeAttributeSet {
    /// Gift id.
    pub gift_id: i64,
    /// Raw attributes artifact.
    pub raw_attributes: FileArtifact,
    /// Attribute count.
    pub attribute_count: i32,
    /// Models.
    pub models: Vec<UpgradeModel>,
    /// Patterns.
    pub patterns: Vec<UpgradePattern>,
    /// Backdrops.
    pub backdrops: Vec<UpgradeBackdrop>,
    /// Document ids.
    pub document_ids: Vec<i64>,
}

impl UpgradeAttributeSet {
    /// Renders the set.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("gift_id", Value::Int(self.gift_id)),
            ("raw_attributes", self.raw_attributes.to_value()),
            (
                "attribute_count",
                Value::Int(i64::from(self.attribute_count)),
            ),
            (
                "models",
                array_or_null(
                    self.models
                        .iter()
                        .map(|model| {
                            Value::object(vec![
                                ("name", Value::Str(model.name.clone())),
                                ("document_id", Value::Int(model.document_id)),
                                ("crafted", Value::Bool(model.crafted)),
                                ("rarity", model.rarity.to_value()),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "patterns",
                array_or_null(
                    self.patterns
                        .iter()
                        .map(|pattern| {
                            Value::object(vec![
                                ("name", Value::Str(pattern.name.clone())),
                                ("document_id", Value::Int(pattern.document_id)),
                                ("rarity", pattern.rarity.to_value()),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "backdrops",
                array_or_null(
                    self.backdrops
                        .iter()
                        .map(|backdrop| {
                            Value::object(vec![
                                ("name", Value::Str(backdrop.name.clone())),
                                ("backdrop_id", Value::Int(i64::from(backdrop.backdrop_id))),
                                ("center_color", Value::Int(i64::from(backdrop.center_color))),
                                ("edge_color", Value::Int(i64::from(backdrop.edge_color))),
                                (
                                    "pattern_color",
                                    Value::Int(i64::from(backdrop.pattern_color)),
                                ),
                                ("text_color", Value::Int(i64::from(backdrop.text_color))),
                                ("rarity", backdrop.rarity.to_value()),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "document_ids",
                array_or_null(self.document_ids.iter().map(|id| Value::Int(*id)).collect()),
            ),
        ])
    }
}

/// One missing thumbnail.
#[derive(Debug, Clone, Default)]
pub struct MissingThumb {
    /// Kind (`photo` or `video`).
    pub kind: String,
    /// Thumb type.
    pub type_: String,
    /// Expected size.
    pub expected_size: i64,
    /// Error message.
    pub error: String,
}

impl MissingThumb {
    /// Renders the entry.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("kind", Value::Str(self.kind.clone())),
            ("type", Value::Str(self.type_.clone())),
            ("expected_size", Value::Int(self.expected_size)),
            ("error", Value::Str(self.error.clone())),
        ])
    }
}

/// One document's export.
#[derive(Debug, Clone, Default)]
pub struct DocumentManifest {
    /// Document id.
    pub id: i64,
    /// Unix date.
    pub date: i32,
    /// Datacenter id.
    pub dc_id: i32,
    /// MIME type.
    pub mime_type: String,
    /// Expected size.
    pub expected_size: i64,
    /// File name.
    pub file_name: String,
    /// Sticker alt text.
    pub sticker_alt: String,
    /// Purposes.
    pub purposes: Vec<String>,
    /// File artifact.
    pub file: FileArtifact,
    /// Animation validated flag.
    pub animation_validated: bool,
    /// Validation error.
    pub validation_error: String,
    /// Thumbnails.
    pub thumbs: Vec<FileArtifact>,
    /// Missing thumbnails.
    pub missing_thumbs: Vec<MissingThumb>,
}

impl DocumentManifest {
    /// Renders the document entry.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut fields: Vec<(&str, Value)> = vec![
            ("id", Value::Int(self.id)),
            ("date", Value::Int(i64::from(self.date))),
            ("dc_id", Value::Int(i64::from(self.dc_id))),
            ("mime_type", Value::Str(self.mime_type.clone())),
            ("expected_size", Value::Int(self.expected_size)),
        ];
        push_str_opt(&mut fields, "file_name", &self.file_name);
        push_str_opt(&mut fields, "sticker_alt", &self.sticker_alt);
        fields.push((
            "purposes",
            Value::Array(
                self.purposes
                    .iter()
                    .map(|p| Value::Str(p.clone()))
                    .collect(),
            ),
        ));
        fields.push(("file", self.file.to_value()));
        if self.animation_validated {
            fields.push(("animation_validated", Value::Bool(true)));
        }
        push_str_opt(&mut fields, "validation_error", &self.validation_error);
        if !self.thumbs.is_empty() {
            fields.push((
                "thumbs",
                Value::Array(self.thumbs.iter().map(FileArtifact::to_value).collect()),
            ));
        }
        if !self.missing_thumbs.is_empty() {
            fields.push((
                "missing_thumbs",
                Value::Array(
                    self.missing_thumbs
                        .iter()
                        .map(MissingThumb::to_value)
                        .collect(),
                ),
            ));
        }
        Value::object(fields)
    }
}

/// The `manifest.json` root.
#[derive(Debug, Clone, Default)]
pub struct CatalogManifest {
    /// Schema version.
    pub schema: i32,
    /// Catalog hash.
    pub hash: i32,
    /// Raw catalog artifact.
    pub raw_catalog: FileArtifact,
    /// Gift count.
    pub gift_count: i32,
    /// Chat count.
    pub chat_count: i32,
    /// User count.
    pub user_count: i32,
    /// Upgradeable gift count.
    pub upgradeable_gift_count: i32,
    /// Upgrade attribute set count.
    pub upgrade_attribute_set_count: i32,
    /// Upgrade attribute count.
    pub upgrade_attribute_count: i32,
    /// Upgrade model count.
    pub upgrade_model_count: i32,
    /// Upgrade pattern count.
    pub upgrade_pattern_count: i32,
    /// Upgrade backdrop count.
    pub upgrade_backdrop_count: i32,
    /// Missing thumbnail count.
    pub missing_thumb_count: i32,
    /// Gifts.
    pub gifts: Vec<GiftManifest>,
    /// Upgrade attribute sets.
    pub upgrade_attribute_sets: Vec<UpgradeAttributeSet>,
    /// Documents.
    pub documents: Vec<DocumentManifest>,
    /// Total document bytes.
    pub total_document_bytes: i64,
    /// Boundary note.
    pub boundary_note: String,
}

impl CatalogManifest {
    /// Renders the manifest.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("schema", Value::Int(i64::from(self.schema))),
            ("hash", Value::Int(i64::from(self.hash))),
            ("raw_catalog", self.raw_catalog.to_value()),
            ("gift_count", Value::Int(i64::from(self.gift_count))),
            ("chat_count", Value::Int(i64::from(self.chat_count))),
            ("user_count", Value::Int(i64::from(self.user_count))),
            (
                "upgradeable_gift_count",
                Value::Int(i64::from(self.upgradeable_gift_count)),
            ),
            (
                "upgrade_attribute_set_count",
                Value::Int(i64::from(self.upgrade_attribute_set_count)),
            ),
            (
                "upgrade_attribute_count",
                Value::Int(i64::from(self.upgrade_attribute_count)),
            ),
            (
                "upgrade_model_count",
                Value::Int(i64::from(self.upgrade_model_count)),
            ),
            (
                "upgrade_pattern_count",
                Value::Int(i64::from(self.upgrade_pattern_count)),
            ),
            (
                "upgrade_backdrop_count",
                Value::Int(i64::from(self.upgrade_backdrop_count)),
            ),
            (
                "missing_thumb_count",
                Value::Int(i64::from(self.missing_thumb_count)),
            ),
            (
                "gifts",
                array_or_null(self.gifts.iter().map(GiftManifest::to_value).collect()),
            ),
            (
                "upgrade_attribute_sets",
                array_or_null(
                    self.upgrade_attribute_sets
                        .iter()
                        .map(UpgradeAttributeSet::to_value)
                        .collect(),
                ),
            ),
            (
                "documents",
                array_or_null(
                    self.documents
                        .iter()
                        .map(DocumentManifest::to_value)
                        .collect(),
                ),
            ),
            (
                "total_document_bytes",
                Value::Int(self.total_document_bytes),
            ),
            ("boundary_note", Value::Str(self.boundary_note.clone())),
        ])
    }
}

/// Pushes an integer field when non-zero (`omitempty`).
fn push_int_opt(fields: &mut Vec<(&'static str, Value)>, key: &'static str, value: i64) {
    if value != 0 {
        fields.push((key, Value::Int(value)));
    }
}

/// Pushes a string field when non-empty (`omitempty`).
fn push_str_opt(fields: &mut Vec<(&'static str, Value)>, key: &'static str, value: &str) {
    if !value.is_empty() {
        fields.push((key, Value::Str(value.to_owned())));
    }
}

/// Pushes a boolean field when true (`omitempty`).
fn push_bool_opt(fields: &mut Vec<(&'static str, Value)>, key: &'static str, value: bool) {
    if value {
        fields.push((key, Value::Bool(true)));
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
