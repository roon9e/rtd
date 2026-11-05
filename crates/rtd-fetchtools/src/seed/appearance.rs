//! `appearancefetch` seed values: the appearance catalog.
//!
//! Mirrors the Go `internal/seed/appearance` structs and the conversions in
//! `cmd/appearancefetch`, including which slices Go builds by `append` (and so
//! marshals as `null` when empty) versus by `make` (marshalled as `[]`).

use crate::gojson::Value;

/// One document attribute (`image_size` or `filename`).
#[derive(Debug, Clone, Default)]
pub struct Attribute {
    /// Attribute kind (`image_size` or `filename`).
    pub kind: String,
    /// Image width.
    pub w: i32,
    /// Image height.
    pub h: i32,
    /// File name.
    pub file_name: String,
}

/// One downloaded thumbnail rendition.
#[derive(Debug, Clone, Default)]
pub struct Photo {
    /// Entry kind (`size`).
    pub kind: String,
    /// Thumbnail type.
    pub type_: String,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
    /// Byte size.
    pub size: i32,
    /// Relative file path.
    pub path: String,
    /// SHA-256 of the file.
    pub sha256: String,
}

/// A downloaded wallpaper/theme document.
#[derive(Debug, Clone, Default)]
pub struct Document {
    /// Document id.
    pub id: i64,
    /// Access hash.
    pub access_hash: i64,
    /// Unix date.
    pub date: i32,
    /// MIME type.
    pub mime_type: String,
    /// Byte size.
    pub size: i64,
    /// Datacenter id.
    pub dc_id: i32,
    /// Relative body path.
    pub path: String,
    /// SHA-256 of the body.
    pub sha256: String,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Thumbnails.
    pub thumbs: Vec<Photo>,
}

/// Wallpaper colour settings.
#[derive(Debug, Clone, Default)]
pub struct WallpaperSettings {
    /// Blur.
    pub blur: bool,
    /// Motion.
    pub motion: bool,
    /// Background colour.
    pub background_color: i32,
    /// Second background colour.
    pub second_background_color: i32,
    /// Third background colour.
    pub third_background_color: i32,
    /// Fourth background colour.
    pub fourth_background_color: i32,
    /// Intensity.
    pub intensity: i32,
    /// Rotation.
    pub rotation: i32,
}

/// A wallpaper (file-backed or colour-only).
#[derive(Debug, Clone, Default)]
pub struct Wallpaper {
    /// Wallpaper id.
    pub id: i64,
    /// Access hash.
    pub access_hash: i64,
    /// Type (0 file-backed, 1 colour-only).
    pub type_: i32,
    /// Default wallpaper flag.
    pub default: bool,
    /// Pattern flag.
    pub pattern: bool,
    /// Dark flag.
    pub dark: bool,
    /// Slug.
    pub slug: String,
    /// Settings.
    pub settings: WallpaperSettings,
    /// Document.
    pub document: Document,
}

/// One theme settings entry.
#[derive(Debug, Clone, Default)]
pub struct ThemeSettings {
    /// Base theme name.
    pub base_theme: String,
    /// Accent colour.
    pub accent_color: i32,
    /// Outbox accent colour.
    pub outbox_accent_color: i32,
    /// Message colours.
    pub message_colors: Option<Vec<i32>>,
    /// Wallpaper.
    pub wallpaper: Wallpaper,
}

/// One chat theme.
#[derive(Debug, Clone, Default)]
pub struct ChatTheme {
    /// Theme id.
    pub id: i64,
    /// Access hash.
    pub access_hash: i64,
    /// Slug.
    pub slug: String,
    /// Title.
    pub title: String,
    /// Emoticon.
    pub emoticon: String,
    /// Default flag.
    pub default: bool,
    /// Settings.
    pub settings: Vec<ThemeSettings>,
}

/// A peer colour set.
#[derive(Debug, Clone, Default)]
pub struct ColorSet {
    /// Predicate name.
    pub predicate_name: String,
    /// Colours.
    pub colors: Option<Vec<i32>>,
    /// Palette colours.
    pub palette_colors: Option<Vec<i32>>,
    /// Background colours.
    pub bg_colors: Option<Vec<i32>>,
    /// Story colours.
    pub story_colors: Option<Vec<i32>>,
}

/// A peer colour option.
#[derive(Debug, Clone, Default)]
pub struct ColorOption {
    /// Colour id.
    pub id: i32,
    /// Hidden flag.
    pub hidden: bool,
    /// Channel minimum level.
    pub channel_min_level: i32,
    /// Group minimum level.
    pub group_min_level: i32,
    /// Light colours.
    pub colors: Option<ColorSet>,
    /// Dark colours.
    pub dark_colors: Option<ColorSet>,
}

/// The appearance catalog root.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    /// Source label.
    pub source: String,
    /// Export timestamp (RFC 3339 UTC).
    pub exported_at: String,
    /// Server label.
    pub server: String,
    /// Chat themes.
    pub chat_themes: Vec<ChatTheme>,
    /// Wallpapers.
    pub wallpapers: Vec<Wallpaper>,
    /// Peer colours (`None` renders as `null`).
    pub peer_colors: Option<Vec<ColorOption>>,
    /// Peer profile colours (`None` renders as `null`).
    pub peer_profile_colors: Option<Vec<ColorOption>>,
}

impl Catalog {
    /// Renders the catalog as an appearance seed value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("source", Value::Str(self.source.clone())),
            ("exported_at", Value::Str(self.exported_at.clone())),
            (
                "notes",
                Value::object(vec![
                    ("server", Value::Str(self.server.clone())),
                    ("themes_total", Value::Int(0)),
                    (
                        "chat_themes_total",
                        Value::Int(self.chat_themes.len() as i64),
                    ),
                    ("chat_theme_seed", Value::Str(String::new())),
                    (
                        "wallpaper_documents_bucket",
                        Value::Str("documents".to_owned()),
                    ),
                    ("wallpaper_thumbs_bucket", Value::Str("thumbs".to_owned())),
                ]),
            ),
            (
                "chat_themes",
                array_or_null(self.chat_themes.iter().map(chat_theme_value).collect()),
            ),
            (
                "wallpapers",
                array_or_null(self.wallpapers.iter().map(wallpaper_value).collect()),
            ),
            ("peer_colors", option_array(self.peer_colors.as_ref())),
            (
                "peer_profile_colors",
                option_array(self.peer_profile_colors.as_ref()),
            ),
        ])
    }
}

/// Renders one [`ChatTheme`].
#[must_use]
pub fn chat_theme_value(theme: &ChatTheme) -> Value {
    Value::object(vec![
        ("id", Value::Int(theme.id)),
        ("access_hash", Value::Int(theme.access_hash)),
        ("slug", Value::Str(theme.slug.clone())),
        ("title", Value::Str(theme.title.clone())),
        ("emoticon", Value::Str(theme.emoticon.clone())),
        ("default", Value::Bool(theme.default)),
        (
            "settings",
            array_or_null(theme.settings.iter().map(theme_settings_value).collect()),
        ),
    ])
}

/// Renders one [`ThemeSettings`].
#[must_use]
pub fn theme_settings_value(settings: &ThemeSettings) -> Value {
    Value::object(vec![
        ("base_theme", Value::Str(settings.base_theme.clone())),
        ("accent_color", Value::Int(i64::from(settings.accent_color))),
        (
            "outbox_accent_color",
            Value::Int(i64::from(settings.outbox_accent_color)),
        ),
        (
            "message_colors",
            option_ints(settings.message_colors.as_ref()),
        ),
        ("wallpaper", wallpaper_value(&settings.wallpaper)),
    ])
}

/// Renders one [`Wallpaper`].
#[must_use]
pub fn wallpaper_value(wallpaper: &Wallpaper) -> Value {
    Value::object(vec![
        ("id", Value::Int(wallpaper.id)),
        ("access_hash", Value::Int(wallpaper.access_hash)),
        ("type", Value::Int(i64::from(wallpaper.type_))),
        ("default", Value::Bool(wallpaper.default)),
        ("pattern", Value::Bool(wallpaper.pattern)),
        ("dark", Value::Bool(wallpaper.dark)),
        ("slug", Value::Str(wallpaper.slug.clone())),
        ("settings", wallpaper_settings_value(&wallpaper.settings)),
        ("document", document_value(&wallpaper.document)),
    ])
}

/// Renders wallpaper colour settings (`id` is always 0, as in Go).
#[must_use]
pub fn wallpaper_settings_value(settings: &WallpaperSettings) -> Value {
    Value::object(vec![
        ("id", Value::Int(0)),
        ("blur", Value::Bool(settings.blur)),
        ("motion", Value::Bool(settings.motion)),
        (
            "background_color",
            Value::Int(i64::from(settings.background_color)),
        ),
        (
            "second_background_color",
            Value::Int(i64::from(settings.second_background_color)),
        ),
        (
            "third_background_color",
            Value::Int(i64::from(settings.third_background_color)),
        ),
        (
            "fourth_background_color",
            Value::Int(i64::from(settings.fourth_background_color)),
        ),
        ("intensity", Value::Int(i64::from(settings.intensity))),
        ("rotation", Value::Int(i64::from(settings.rotation))),
    ])
}

/// Renders one [`Document`].
#[must_use]
pub fn document_value(document: &Document) -> Value {
    Value::object(vec![
        ("id", Value::Int(document.id)),
        ("access_hash", Value::Int(document.access_hash)),
        ("date", Value::Int(i64::from(document.date))),
        ("mime_type", Value::Str(document.mime_type.clone())),
        ("size", Value::Int(document.size)),
        ("dc_id", Value::Int(i64::from(document.dc_id))),
        ("path", Value::Str(document.path.clone())),
        ("sha256", Value::Str(document.sha256.clone())),
        (
            "attributes",
            array_or_null(document.attributes.iter().map(attribute_value).collect()),
        ),
        (
            "thumbs",
            array_or_null(document.thumbs.iter().map(photo_value).collect()),
        ),
    ])
}

/// Renders one [`Attribute`].
#[must_use]
pub fn attribute_value(attribute: &Attribute) -> Value {
    let mut fields: Vec<(&str, Value)> = vec![("kind", Value::Str(attribute.kind.clone()))];
    if attribute.w != 0 {
        fields.push(("w", Value::Int(i64::from(attribute.w))));
    }
    if attribute.h != 0 {
        fields.push(("h", Value::Int(i64::from(attribute.h))));
    }
    if !attribute.file_name.is_empty() {
        fields.push(("file_name", Value::Str(attribute.file_name.clone())));
    }
    Value::object(fields)
}

/// Renders one [`Photo`].
#[must_use]
pub fn photo_value(photo: &Photo) -> Value {
    let mut fields: Vec<(&str, Value)> = vec![
        ("kind", Value::Str(photo.kind.clone())),
        ("type", Value::Str(photo.type_.clone())),
    ];
    if photo.w != 0 {
        fields.push(("w", Value::Int(i64::from(photo.w))));
    }
    if photo.h != 0 {
        fields.push(("h", Value::Int(i64::from(photo.h))));
    }
    if photo.size != 0 {
        fields.push(("size", Value::Int(i64::from(photo.size))));
    }
    if !photo.path.is_empty() {
        fields.push(("path", Value::Str(photo.path.clone())));
    }
    if !photo.sha256.is_empty() {
        fields.push(("sha256", Value::Str(photo.sha256.clone())));
    }
    Value::object(fields)
}

/// Renders one [`ColorOption`].
#[must_use]
pub fn color_option_value(option: &ColorOption) -> Value {
    Value::object(vec![
        ("id", Value::Int(i64::from(option.id))),
        ("hidden", Value::Bool(option.hidden)),
        (
            "channel_min_level",
            Value::Int(i64::from(option.channel_min_level)),
        ),
        (
            "group_min_level",
            Value::Int(i64::from(option.group_min_level)),
        ),
        ("colors", option_colors(option.colors.as_ref())),
        ("dark_colors", option_colors(option.dark_colors.as_ref())),
    ])
}

/// Renders one [`ColorSet`].
#[must_use]
pub fn color_set_value(set: &ColorSet) -> Value {
    Value::object(vec![
        ("predicate_name", Value::Str(set.predicate_name.clone())),
        ("colors", option_ints(set.colors.as_ref())),
        ("palette_colors", option_ints(set.palette_colors.as_ref())),
        ("bg_colors", option_ints(set.bg_colors.as_ref())),
        ("story_colors", option_ints(set.story_colors.as_ref())),
    ])
}

/// Renders an append-built slice: `null` when empty.
fn array_or_null(values: Vec<Value>) -> Value {
    if values.is_empty() {
        Value::Null
    } else {
        Value::Array(values)
    }
}

/// Renders an optional slice: `null` when absent.
fn option_array(values: Option<&Vec<ColorOption>>) -> Value {
    match values {
        Some(list) => Value::Array(list.iter().map(color_option_value).collect()),
        None => Value::Null,
    }
}

/// Renders an optional colour-set pointer: `null` when absent.
fn option_colors(set: Option<&ColorSet>) -> Value {
    match set {
        Some(set) => color_set_value(set),
        None => Value::Null,
    }
}

/// Renders an optional integer slice: `null` when absent.
fn option_ints(values: Option<&Vec<i32>>) -> Value {
    match values {
        Some(list) => Value::from(list.clone()),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Attribute, Catalog, ChatTheme, ColorOption, ColorSet, Document, Photo, ThemeSettings,
        Wallpaper, WallpaperSettings,
    };

    #[test]
    fn renders_like_go_appearance_seed() {
        let doc = Document {
            id: 10,
            access_hash: 20,
            date: 1_700_000_000,
            mime_type: "image/jpeg".to_owned(),
            size: 100,
            dc_id: 2,
            path: "default_wallpapers/documents/10.dat".to_owned(),
            sha256: "abc".to_owned(),
            attributes: vec![
                Attribute {
                    kind: "image_size".to_owned(),
                    w: 800,
                    h: 600,
                    ..Attribute::default()
                },
                Attribute {
                    kind: "filename".to_owned(),
                    file_name: "a.jpg".to_owned(),
                    ..Attribute::default()
                },
            ],
            thumbs: vec![Photo {
                kind: "size".to_owned(),
                type_: "m".to_owned(),
                w: 40,
                h: 30,
                size: 7,
                path: "default_wallpapers/thumbs/m/10.dat".to_owned(),
                sha256: "def".to_owned(),
            }],
        };
        let catalog = Catalog {
            source: "upstream appearance snapshot".to_owned(),
            exported_at: "2023-11-14T22:13:20Z".to_owned(),
            server: "official".to_owned(),
            chat_themes: vec![ChatTheme {
                id: 1,
                access_hash: 2,
                slug: "s".to_owned(),
                title: "t".to_owned(),
                emoticon: String::new(),
                default: true,
                settings: vec![ThemeSettings {
                    base_theme: "night".to_owned(),
                    accent_color: 3,
                    outbox_accent_color: 0,
                    message_colors: Some(vec![5, 6]),
                    wallpaper: Wallpaper {
                        id: 7,
                        type_: 1,
                        dark: true,
                        ..Wallpaper::default()
                    },
                }],
            }],
            wallpapers: vec![Wallpaper {
                id: 10,
                access_hash: 20,
                default: true,
                pattern: true,
                slug: "w".to_owned(),
                settings: WallpaperSettings {
                    blur: true,
                    background_color: 1,
                    ..WallpaperSettings::default()
                },
                document: doc,
                ..Wallpaper::default()
            }],
            peer_colors: Some(vec![ColorOption {
                id: 1,
                hidden: true,
                channel_min_level: 2,
                colors: Some(ColorSet {
                    predicate_name: "help.peerColorSet".to_owned(),
                    colors: Some(vec![9, 8]),
                    ..ColorSet::default()
                }),
                ..ColorOption::default()
            }]),
            peer_profile_colors: Some(vec![ColorOption {
                id: 2,
                colors: Some(ColorSet {
                    predicate_name: "help.peerColorProfileSet".to_owned(),
                    palette_colors: Some(vec![1, 2]),
                    bg_colors: Some(vec![3]),
                    story_colors: Some(vec![4]),
                    ..ColorSet::default()
                }),
                ..ColorOption::default()
            }]),
        };
        assert_eq!(catalog.to_value().render(" "), EXPECTED);
    }

    const EXPECTED: &str = r#"{
 "source": "upstream appearance snapshot",
 "exported_at": "2023-11-14T22:13:20Z",
 "notes": {
  "server": "official",
  "themes_total": 0,
  "chat_themes_total": 1,
  "chat_theme_seed": "",
  "wallpaper_documents_bucket": "documents",
  "wallpaper_thumbs_bucket": "thumbs"
 },
 "chat_themes": [
  {
   "id": 1,
   "access_hash": 2,
   "slug": "s",
   "title": "t",
   "emoticon": "",
   "default": true,
   "settings": [
    {
     "base_theme": "night",
     "accent_color": 3,
     "outbox_accent_color": 0,
     "message_colors": [
      5,
      6
     ],
     "wallpaper": {
      "id": 7,
      "access_hash": 0,
      "type": 1,
      "default": false,
      "pattern": false,
      "dark": true,
      "slug": "",
      "settings": {
       "id": 0,
       "blur": false,
       "motion": false,
       "background_color": 0,
       "second_background_color": 0,
       "third_background_color": 0,
       "fourth_background_color": 0,
       "intensity": 0,
       "rotation": 0
      },
      "document": {
       "id": 0,
       "access_hash": 0,
       "date": 0,
       "mime_type": "",
       "size": 0,
       "dc_id": 0,
       "path": "",
       "sha256": "",
       "attributes": null,
       "thumbs": null
      }
     }
    }
   ]
  }
 ],
 "wallpapers": [
  {
   "id": 10,
   "access_hash": 20,
   "type": 0,
   "default": true,
   "pattern": true,
   "dark": false,
   "slug": "w",
   "settings": {
    "id": 0,
    "blur": true,
    "motion": false,
    "background_color": 1,
    "second_background_color": 0,
    "third_background_color": 0,
    "fourth_background_color": 0,
    "intensity": 0,
    "rotation": 0
   },
   "document": {
    "id": 10,
    "access_hash": 20,
    "date": 1700000000,
    "mime_type": "image/jpeg",
    "size": 100,
    "dc_id": 2,
    "path": "default_wallpapers/documents/10.dat",
    "sha256": "abc",
    "attributes": [
     {
      "kind": "image_size",
      "w": 800,
      "h": 600
     },
     {
      "kind": "filename",
      "file_name": "a.jpg"
     }
    ],
    "thumbs": [
     {
      "kind": "size",
      "type": "m",
      "w": 40,
      "h": 30,
      "size": 7,
      "path": "default_wallpapers/thumbs/m/10.dat",
      "sha256": "def"
     }
    ]
   }
  }
 ],
 "peer_colors": [
  {
   "id": 1,
   "hidden": true,
   "channel_min_level": 2,
   "group_min_level": 0,
   "colors": {
    "predicate_name": "help.peerColorSet",
    "colors": [
     9,
     8
    ],
    "palette_colors": null,
    "bg_colors": null,
    "story_colors": null
   },
   "dark_colors": null
  }
 ],
 "peer_profile_colors": [
  {
   "id": 2,
   "hidden": false,
   "channel_min_level": 0,
   "group_min_level": 0,
   "colors": {
    "predicate_name": "help.peerColorProfileSet",
    "colors": null,
    "palette_colors": [
     1,
     2
    ],
    "bg_colors": [
     3
    ],
    "story_colors": [
     4
    ]
   },
   "dark_colors": null
  }
 ]
}"#;
}
