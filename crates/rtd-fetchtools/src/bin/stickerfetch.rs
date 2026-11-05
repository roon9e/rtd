//! `stickerfetch` — pull sticker sets, effects and reactions from Telegram.
//!
//! Port of `cmd/stickerfetch`. Writes the seed directory layout `telesrv`'s
//! `SeedMedia` imports (`<set>/set_info.json` + `stickers/`, and the effects and
//! reactions export trees).
//!
//! Usage: `stickerfetch [-timeout 12m] <out_dir> <spec> [spec...]`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rtd_fetchtools::client::{Tg, default_session_path};
use rtd_fetchtools::creds::{self, TDESKTOP_API_HASH, TDESKTOP_API_ID};
use rtd_fetchtools::seed::sticker::{
    Attribute, Document, Effect, EffectsFile, InputSetRef, Pack, Reaction, Set, SetInfo, Thumb,
    reactions_value,
};
use rtd_fetchtools::{Result, fsutil, timefmt};
use rtd_tg::{
    AvailableEffect, Document as TgDocument, DocumentAttribute, InputStickerSet,
    MessagesAvailableEffects, MessagesAvailableReactions, MessagesFeaturedStickers,
    MessagesGetAvailableEffects, MessagesGetAvailableReactions, MessagesGetFeaturedEmojiStickers,
    MessagesGetFeaturedStickers, MessagesGetStickerSet, MessagesStickerSet, Method, Object,
    PhotoSize, StickerSetCovered,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ERROR: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // Only `-timeout` is a flag; skip it and its value, keep the rest positional.
    let mut positional: Vec<String> = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if arg == "-timeout" {
            index += 2;
            continue;
        }
        if arg.starts_with("-timeout=") {
            index += 1;
            continue;
        }
        positional.push(arg.clone());
        index += 1;
    }
    if positional.len() < 2 {
        eprintln!(
            "usage: SESSION=/tmp/appearance.session stickerfetch [-timeout 12m] <out_dir> <spec> [spec...]"
        );
        return Ok(());
    }
    let out = PathBuf::from(&positional[0]);
    let specs = &positional[1..];

    let (api_id, api_hash) = creds::api_credentials(TDESKTOP_API_ID, TDESKTOP_API_HASH);
    let session = default_session_path();
    let mut tg = Tg::connect(api_id, &api_hash, &session)?;
    if !tg.is_authorized()? {
        return Err(
            format!("session {session} unauthorized (log in with appearancefetch first)").into(),
        );
    }

    for spec in specs {
        let result = match spec.as_str() {
            "effects" => fetch_effects(&mut tg, &out),
            "reactions" => fetch_reactions(&mut tg, &out),
            "featured_emoji" => fetch_featured(&mut tg, &out, true),
            "featured_stickers" => fetch_featured(&mut tg, &out, false),
            _ => fetch_set(&mut tg, &out, spec),
        };
        if let Err(error) = result {
            eprintln!("skip {spec}: {error}");
        }
    }
    Ok(())
}

/// Maps a set spec to an input set, its Go type name and its fallback label.
fn spec_to_input(spec: &str) -> Result<(InputStickerSet, &'static str, String)> {
    if let Some(name) = spec.strip_prefix("short:") {
        return Ok((
            InputStickerSet::InputStickerSetShortName {
                short_name: name.to_owned(),
            },
            "*tg.InputStickerSetShortName",
            name.to_owned(),
        ));
    }
    if let Some(rest) = spec.strip_prefix("id:") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("bad spec {spec:?}, want id:<id>:<access_hash>").into());
        }
        let id = parts[0]
            .parse::<i64>()
            .map_err(|_| format!("bad spec {spec:?}"))?;
        let hash = parts[1]
            .parse::<i64>()
            .map_err(|_| format!("bad spec {spec:?}"))?;
        return Ok((
            InputStickerSet::InputStickerSetID {
                id,
                access_hash: hash,
            },
            "*tg.InputStickerSetID",
            String::new(),
        ));
    }
    if let Some(emoticon) = spec.strip_prefix("dice:") {
        return Ok((
            InputStickerSet::InputStickerSetDice {
                emoticon: emoticon.to_owned(),
            },
            "*tg.InputStickerSetDice",
            "Dice".to_owned(),
        ));
    }
    let (input, label) = match spec {
        "animated_emoji" => (
            InputStickerSet::InputStickerSetAnimatedEmoji,
            "AnimatedEmoji",
        ),
        "animated_emoji_animations" => (
            InputStickerSet::InputStickerSetAnimatedEmojiAnimations,
            "AnimatedEmojiAnimations",
        ),
        "emoji_generic_animations" => (
            InputStickerSet::InputStickerSetEmojiGenericAnimations,
            "EmojiGenericAnimations",
        ),
        "emoji_default_statuses" => (
            InputStickerSet::InputStickerSetEmojiDefaultStatuses,
            "EmojiDefaultStatuses",
        ),
        "emoji_channel_default_statuses" => (
            InputStickerSet::InputStickerSetEmojiChannelDefaultStatuses,
            "EmojiChannelDefaultStatuses",
        ),
        "emoji_default_topic_icons" => (
            InputStickerSet::InputStickerSetEmojiDefaultTopicIcons,
            "EmojiDefaultTopicIcons",
        ),
        "premium_gifts" => (InputStickerSet::InputStickerSetPremiumGifts, "PremiumGifts"),
        "ton_gifts" => (InputStickerSet::InputStickerSetTonGifts, "TonGifts"),
        other => return Err(format!("unknown spec {other:?}").into()),
    };
    Ok((input, go_input_type(spec), label.to_owned()))
}

/// The Go `%T` name for a built-in input set spec.
fn go_input_type(spec: &str) -> &'static str {
    match spec {
        "animated_emoji" => "*tg.InputStickerSetAnimatedEmoji",
        "animated_emoji_animations" => "*tg.InputStickerSetAnimatedEmojiAnimations",
        "emoji_generic_animations" => "*tg.InputStickerSetEmojiGenericAnimations",
        "emoji_default_statuses" => "*tg.InputStickerSetEmojiDefaultStatuses",
        "emoji_channel_default_statuses" => "*tg.InputStickerSetEmojiChannelDefaultStatuses",
        "emoji_default_topic_icons" => "*tg.InputStickerSetEmojiDefaultTopicIcons",
        "premium_gifts" => "*tg.InputStickerSetPremiumGifts",
        "ton_gifts" => "*tg.InputStickerSetTonGifts",
        _ => "*tg.InputStickerSetEmpty",
    }
}

/// A document location without the thumb size.
#[derive(Clone, Copy)]
struct DocRef<'a> {
    id: i64,
    access_hash: i64,
    file_reference: &'a [u8],
    dc_id: i32,
}

/// Fetches and exports one sticker set.
fn fetch_set(tg: &mut Tg, out_root: &Path, spec: &str) -> Result<()> {
    let (input, go_type, label) = spec_to_input(spec)?;
    let request = Method::MessagesGetStickerSet(Box::new(MessagesGetStickerSet {
        stickerset: Box::new(input),
        hash: 0,
    }));
    let Object::MessagesStickerSet(response) = tg.invoke(request)? else {
        return Err(format!("got an unexpected response for {spec}").into());
    };
    let MessagesStickerSet::StickerSet {
        set,
        packs,
        documents,
        ..
    } = &*response
    else {
        return Err(format!("got a not-modified response for {spec}").into());
    };
    let set_name = if set.short_name.is_empty() {
        label
    } else {
        set.short_name.clone()
    };
    let set_dir = out_root.join(format!("{set_name}_{}", set.id));
    let stickers_dir = set_dir.join("stickers");
    std::fs::create_dir_all(&stickers_dir)?;

    println!(
        "[{spec}] set={set_name:?} id={} docs={} downloading...",
        set.id,
        documents.len()
    );
    let mut exported = Vec::with_capacity(documents.len());
    for document in documents {
        exported.push(download_document(tg, &stickers_dir, document)?);
    }

    let mut exported_packs = Vec::with_capacity(packs.len());
    for pack in packs {
        exported_packs.push(Pack {
            emoticon: pack.emoticon.clone(),
            documents: pack.documents.clone(),
        });
    }
    let info = SetInfo {
        api_call: "messages.getStickerSet".to_owned(),
        input_set_type: go_type.to_owned(),
        set: Set {
            id: set.id,
            access_hash: set.access_hash,
            title: set.title.clone(),
            short_name: set.short_name.clone(),
            count: set.count,
            hash: set.hash,
            official: set.official,
            masks: set.masks,
            emojis: set.emojis,
            thumbs: inline_thumbs(set.thumbs.as_deref()),
            thumb_dc_id: set.thumb_dc_id.unwrap_or(0),
            thumb_version: set.thumb_version.unwrap_or(0),
            thumb_document_id: set.thumb_document_id,
        },
        packs: exported_packs,
        documents: exported,
    };
    let body = format!("{}\n", info.to_value().render("  "));
    fsutil::write_file_atomic(&set_dir.join("set_info.json"), body.as_bytes())?;
    println!(
        "[{spec}] set={set_name:?} id={} docs={} masks={} emojis={}",
        set.id,
        info.documents.len(),
        set.masks,
        set.emojis
    );
    Ok(())
}

/// Downloads a document body to `<dir>/<id><ext>` and exports its thumbs.
fn download_document(tg: &mut Tg, dir: &Path, class: &TgDocument) -> Result<Document> {
    let TgDocument::Document {
        id,
        access_hash,
        file_reference,
        date,
        mime_type,
        size,
        dc_id,
        attributes,
        ..
    } = class
    else {
        return Ok(Document::default());
    };
    let reference = DocRef {
        id: *id,
        access_hash: *access_hash,
        file_reference,
        dc_id: *dc_id,
    };
    let ext = document_extension(mime_type);
    let path = dir.join(format!("{id}{ext}"));
    if !complete_existing(&path, *size) {
        download_to_file(tg, reference, "", *size, &path)?;
    }
    let exported_thumbs = export_thumbs(tg, dir, class)?;
    Ok(Document {
        id: *id,
        access_hash: *access_hash,
        file_reference: hex::encode(file_reference),
        date: timefmt::rfc3339_utc(i64::from(*date)),
        mime_type: mime_type.clone(),
        size: *size,
        dc_id: *dc_id,
        attributes: attributes.iter().filter_map(convert_attribute).collect(),
        thumbs: exported_thumbs,
    })
}

/// Maps a document attribute to the seed form.
fn convert_attribute(attribute: &DocumentAttribute) -> Option<Attribute> {
    match attribute {
        DocumentAttribute::DocumentAttributeImageSize { w, h } => Some(Attribute {
            type_: "DocumentAttributeImageSize".to_owned(),
            w: *w,
            h: *h,
            ..Attribute::default()
        }),
        DocumentAttribute::DocumentAttributeAnimated => Some(Attribute {
            type_: "DocumentAttributeAnimated".to_owned(),
            ..Attribute::default()
        }),
        DocumentAttribute::DocumentAttributeSticker {
            alt,
            mask,
            stickerset,
            ..
        } => Some(Attribute {
            type_: "DocumentAttributeSticker".to_owned(),
            alt: alt.clone(),
            mask: *mask,
            stickerset: input_set_ref(stickerset),
            ..Attribute::default()
        }),
        DocumentAttribute::DocumentAttributeCustomEmoji {
            alt,
            free,
            text_color,
            stickerset,
            ..
        } => Some(Attribute {
            type_: "DocumentAttributeCustomEmoji".to_owned(),
            alt: alt.clone(),
            free: *free,
            text_color: *text_color,
            stickerset: input_set_ref(stickerset),
            ..Attribute::default()
        }),
        DocumentAttribute::DocumentAttributeVideo {
            w,
            h,
            duration,
            round_message,
            supports_streaming,
            ..
        } => Some(Attribute {
            type_: "DocumentAttributeVideo".to_owned(),
            w: *w,
            h: *h,
            duration: *duration,
            round_message: *round_message,
            supports_streaming: *supports_streaming,
            ..Attribute::default()
        }),
        DocumentAttribute::DocumentAttributeFilename { file_name } => Some(Attribute {
            type_: "DocumentAttributeFilename".to_owned(),
            file_name: file_name.clone(),
            ..Attribute::default()
        }),
        _ => None,
    }
}

/// Returns an input-set reference when the set is an `InputStickerSetID`.
fn input_set_ref(set: &InputStickerSet) -> Option<InputSetRef> {
    match set {
        InputStickerSet::InputStickerSetID { id, access_hash } => Some(InputSetRef {
            id: *id,
            access_hash: *access_hash,
        }),
        _ => None,
    }
}

/// Exports a document's thumbnails, downloading file-backed renditions.
fn export_thumbs(tg: &mut Tg, dir: &Path, class: &TgDocument) -> Result<Vec<Thumb>> {
    let TgDocument::Document {
        id,
        access_hash,
        file_reference,
        dc_id,
        thumbs,
        ..
    } = class
    else {
        return Ok(Vec::new());
    };
    let Some(thumbs) = thumbs else {
        return Ok(Vec::new());
    };
    let reference = DocRef {
        id: *id,
        access_hash: *access_hash,
        file_reference,
        dc_id: *dc_id,
    };
    let mut out = Vec::with_capacity(thumbs.len());
    for (index, thumb) in thumbs.iter().enumerate() {
        match thumb {
            PhotoSize::PhotoPathSize { type_, bytes } => out.push(Thumb {
                type_: "PhotoPathSize".to_owned(),
                size_type: type_.clone(),
                bytes: hex::encode(bytes),
                ..Thumb::default()
            }),
            PhotoSize::PhotoStrippedSize { type_, bytes } => out.push(Thumb {
                type_: "PhotoStrippedSize".to_owned(),
                size_type: type_.clone(),
                bytes: hex::encode(bytes),
                ..Thumb::default()
            }),
            PhotoSize::PhotoCachedSize {
                type_, w, h, bytes, ..
            } => out.push(Thumb {
                type_: "PhotoCachedSize".to_owned(),
                size_type: type_.clone(),
                w: *w,
                h: *h,
                bytes: hex::encode(bytes),
                ..Thumb::default()
            }),
            PhotoSize::PhotoSize { type_, w, h, size } => {
                let path = thumb_path(dir, *id, index, type_);
                if !complete_existing(&path, i64::from(*size)) {
                    download_to_file(tg, reference, type_, i64::from(*size), &path)?;
                }
                out.push(Thumb {
                    type_: "PhotoSize".to_owned(),
                    size_type: type_.clone(),
                    w: *w,
                    h: *h,
                    size: *size,
                    ..Thumb::default()
                });
            }
            PhotoSize::PhotoSizeProgressive { type_, w, h, sizes } => {
                if sizes.is_empty() {
                    return Err(format!("document {id} progressive thumb {type_}: no sizes").into());
                }
                let size = *sizes.last().unwrap_or(&0);
                let path = thumb_path(dir, *id, index, type_);
                if !complete_existing(&path, i64::from(size)) {
                    download_to_file(tg, reference, type_, i64::from(size), &path)?;
                }
                out.push(Thumb {
                    type_: "PhotoSizeProgressive".to_owned(),
                    size_type: type_.clone(),
                    w: *w,
                    h: *h,
                    size,
                    sizes: sizes.clone(),
                    ..Thumb::default()
                });
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Builds the seed-side thumb file name.
fn thumb_path(dir: &Path, id: i64, index: usize, thumb_type: &str) -> PathBuf {
    dir.join(format!("{id}_thumb{index}_PhotoSize_type{thumb_type}.jpg"))
}

/// Keeps only inline thumbnails (`path`/`stripped`/`cached`) for a set.
fn inline_thumbs(thumbs: Option<&[PhotoSize]>) -> Vec<Thumb> {
    let Some(thumbs) = thumbs else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for thumb in thumbs {
        match thumb {
            PhotoSize::PhotoPathSize { type_, bytes } => out.push(Thumb {
                type_: "PhotoPathSize".to_owned(),
                size_type: type_.clone(),
                bytes: hex::encode(bytes),
                ..Thumb::default()
            }),
            PhotoSize::PhotoStrippedSize { type_, bytes } => out.push(Thumb {
                type_: "PhotoStrippedSize".to_owned(),
                size_type: type_.clone(),
                bytes: hex::encode(bytes),
                ..Thumb::default()
            }),
            PhotoSize::PhotoCachedSize {
                type_, w, h, bytes, ..
            } => out.push(Thumb {
                type_: "PhotoCachedSize".to_owned(),
                size_type: type_.clone(),
                w: *w,
                h: *h,
                bytes: hex::encode(bytes),
                ..Thumb::default()
            }),
            _ => {}
        }
    }
    out
}

/// Fetches `messages.getAvailableEffects`.
fn fetch_effects(tg: &mut Tg, out_root: &Path) -> Result<()> {
    let request =
        Method::MessagesGetAvailableEffects(Box::new(MessagesGetAvailableEffects { hash: 0 }));
    let Object::MessagesAvailableEffects(response) = tg.invoke(request)? else {
        return Err("getAvailableEffects returned an unexpected response".into());
    };
    let MessagesAvailableEffects::AvailableEffects {
        effects, documents, ..
    } = &*response
    else {
        return Err("getAvailableEffects returned not-modified".into());
    };
    let dir = out_root.join("telegram_effects_export");
    let docs_dir = dir.join("documents");
    std::fs::create_dir_all(&docs_dir)?;
    let mut exported = Vec::with_capacity(documents.len());
    for document in documents {
        exported.push(download_document(tg, &docs_dir, document)?);
    }
    let file = EffectsFile {
        api_call: "messages.getAvailableEffects".to_owned(),
        effects: effects.iter().map(convert_effect).collect(),
        documents: exported,
    };
    let body = format!("{}\n", file.to_value().render("  "));
    fsutil::write_file_atomic(&dir.join("effects.json"), body.as_bytes())?;
    println!(
        "[effects] effects={} documents={}",
        file.effects.len(),
        file.documents.len()
    );
    Ok(())
}

/// Converts one available effect.
fn convert_effect(effect: &AvailableEffect) -> Effect {
    Effect {
        id: effect.id,
        emoticon: effect.emoticon.clone(),
        static_icon_id: effect.static_icon_id,
        effect_sticker_id: effect.effect_sticker_id,
        effect_animation_id: effect.effect_animation_id,
        premium_required: effect.premium_required,
    }
}

/// Fetches `messages.getAvailableReactions`.
fn fetch_reactions(tg: &mut Tg, out_root: &Path) -> Result<()> {
    let request =
        Method::MessagesGetAvailableReactions(Box::new(MessagesGetAvailableReactions { hash: 0 }));
    let Object::MessagesAvailableReactions(response) = tg.invoke(request)? else {
        return Err("getAvailableReactions returned an unexpected response".into());
    };
    let MessagesAvailableReactions::AvailableReactions { reactions, .. } = &*response else {
        return Err("getAvailableReactions returned not-modified".into());
    };
    let dir = out_root.join("telegram_reactions_export");
    let docs_dir = dir.join("reactions");
    let global_dir = dir.join("global_json");
    std::fs::create_dir_all(&docs_dir)?;
    std::fs::create_dir_all(&global_dir)?;
    let mut seen: BTreeMap<i64, Document> = BTreeMap::new();
    let mut out = Vec::with_capacity(reactions.len());
    for (index, reaction) in reactions.iter().enumerate() {
        let mut converted = Reaction {
            reaction: reaction.reaction.clone(),
            title: reaction.title.clone(),
            inactive: reaction.inactive,
            premium: reaction.premium,
            ..Reaction::default()
        };
        converted.static_icon =
            fetch_reaction_doc(tg, &docs_dir, &mut seen, &reaction.static_icon)?;
        converted.appear_animation =
            fetch_reaction_doc(tg, &docs_dir, &mut seen, &reaction.appear_animation)?;
        converted.select_animation =
            fetch_reaction_doc(tg, &docs_dir, &mut seen, &reaction.select_animation)?;
        converted.activate_animation =
            fetch_reaction_doc(tg, &docs_dir, &mut seen, &reaction.activate_animation)?;
        converted.effect_animation =
            fetch_reaction_doc(tg, &docs_dir, &mut seen, &reaction.effect_animation)?;
        if let Some(document) = &reaction.around_animation {
            converted.around_animation = fetch_reaction_doc(tg, &docs_dir, &mut seen, document)?;
        }
        if let Some(document) = &reaction.center_icon {
            converted.center_icon = fetch_reaction_doc(tg, &docs_dir, &mut seen, document)?;
        }
        out.push(converted);
        println!(
            "[reactions] {}/{} {}",
            index + 1,
            reactions.len(),
            reaction.reaction
        );
    }
    let body = format!("{}\n", reactions_value(&out).render("  "));
    fsutil::write_file_atomic(
        &global_dir.join("available_reactions_raw.json"),
        body.as_bytes(),
    )?;
    println!(
        "[reactions] reactions={} documents={}",
        out.len(),
        seen.len()
    );
    Ok(())
}

/// Downloads one reaction document, deduplicating by id.
fn fetch_reaction_doc(
    tg: &mut Tg,
    dir: &Path,
    seen: &mut BTreeMap<i64, Document>,
    class: &TgDocument,
) -> Result<Option<Document>> {
    let TgDocument::Document { id, .. } = class else {
        return Ok(None);
    };
    if let Some(document) = seen.get(id) {
        return Ok(Some(document.clone()));
    }
    let document = download_document(tg, dir, class)?;
    seen.insert(*id, document.clone());
    Ok(Some(document))
}

/// Fetches `messages.getFeatured[Emoji]Stickers` and exports each set.
fn fetch_featured(tg: &mut Tg, out_root: &Path, emoji: bool) -> Result<()> {
    let request = if emoji {
        Method::MessagesGetFeaturedEmojiStickers(Box::new(MessagesGetFeaturedEmojiStickers {
            hash: 0,
        }))
    } else {
        Method::MessagesGetFeaturedStickers(Box::new(MessagesGetFeaturedStickers { hash: 0 }))
    };
    let Object::MessagesFeaturedStickers(response) = tg.invoke(request)? else {
        return Err("getFeaturedStickers returned an unexpected response".into());
    };
    let MessagesFeaturedStickers::FeaturedStickers { sets, .. } = &*response else {
        return Err("getFeaturedStickers returned not-modified".into());
    };
    println!("[featured] emoji={emoji} sets={}", sets.len());
    for covered in sets {
        let set = covered_set(covered);
        let spec = format!("id:{}:{}", set.id, set.access_hash);
        if let Err(error) = fetch_set(tg, out_root, &spec) {
            eprintln!("skip featured {} ({spec}): {error}", set.short_name);
        }
    }
    Ok(())
}

/// Returns the set inside a covered featured set.
fn covered_set(covered: &StickerSetCovered) -> &rtd_tg::StickerSet {
    match covered {
        StickerSetCovered::StickerSetCovered { set, .. }
        | StickerSetCovered::StickerSetFullCovered { set, .. }
        | StickerSetCovered::StickerSetMultiCovered { set, .. }
        | StickerSetCovered::StickerSetNoCovered { set } => set,
    }
}

/// Downloads a document location and writes it atomically to `path`.
fn download_to_file(
    tg: &mut Tg,
    doc: DocRef<'_>,
    thumb_size: &str,
    size: i64,
    path: &Path,
) -> Result<()> {
    let location = rtd_tg::InputFileLocation::InputDocumentFileLocation {
        id: doc.id,
        access_hash: doc.access_hash,
        file_reference: doc.file_reference.to_vec(),
        thumb_size: thumb_size.to_owned(),
    };
    let data = tg.download(location, usize::try_from(size).unwrap_or(0), doc.dc_id)?;
    fsutil::write_file_atomic(path, &data)
}

/// Returns whether an existing file already matches the expected size.
fn complete_existing(path: &Path, want_size: i64) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.is_dir() {
        return false;
    }
    if want_size > 0 {
        metadata.len() == want_size as u64
    } else {
        metadata.len() > 0
    }
}

/// Maps a MIME type to the seed file extension.
fn document_extension(mime_type: &str) -> &'static str {
    match mime_type {
        "video/webm" => ".webm",
        "image/webp" => ".webp",
        _ => ".tgs",
    }
}

#[cfg(test)]
mod tests {
    use super::spec_to_input;
    use rtd_tg::InputStickerSet;

    #[test]
    fn maps_system_specs() {
        let cases = [
            (
                "emoji_default_statuses",
                "*tg.InputStickerSetEmojiDefaultStatuses",
            ),
            (
                "emoji_channel_default_statuses",
                "*tg.InputStickerSetEmojiChannelDefaultStatuses",
            ),
            (
                "emoji_default_topic_icons",
                "*tg.InputStickerSetEmojiDefaultTopicIcons",
            ),
            ("premium_gifts", "*tg.InputStickerSetPremiumGifts"),
            ("ton_gifts", "*tg.InputStickerSetTonGifts"),
        ];
        for (spec, expected) in cases {
            let (_, go_type, _) = spec_to_input(spec).expect("spec");
            assert_eq!(go_type, expected);
        }
    }

    #[test]
    fn maps_short_and_id_and_dice() {
        let (input, go_type, label) = spec_to_input("short:FestiveFontEmoji").expect("short");
        assert_eq!(go_type, "*tg.InputStickerSetShortName");
        assert_eq!(label, "FestiveFontEmoji");
        assert!(matches!(
            input,
            InputStickerSet::InputStickerSetShortName { short_name } if short_name == "FestiveFontEmoji"
        ));

        let (input, go_type, label) = spec_to_input("id:42:-7").expect("id");
        assert_eq!(go_type, "*tg.InputStickerSetID");
        assert_eq!(label, "");
        assert!(matches!(
            input,
            InputStickerSet::InputStickerSetID {
                id: 42,
                access_hash: -7
            }
        ));

        let (input, go_type, label) = spec_to_input("dice:🎲").expect("dice");
        assert_eq!(go_type, "*tg.InputStickerSetDice");
        assert_eq!(label, "Dice");
        assert!(matches!(
            input,
            InputStickerSet::InputStickerSetDice { emoticon } if emoticon == "🎲"
        ));
    }

    #[test]
    fn rejects_bad_specs() {
        for bad in ["id:42", "id:x:1", "id:1:2:3", "unknown"] {
            assert!(spec_to_input(bad).is_err(), "{bad} should be rejected");
        }
    }
}
