//! `appearancefetch` — pull wallpapers and chat themes from official Telegram.
//!
//! Port of `cmd/appearancefetch`. Generates an appearance seed
//! (`default_appearance_seed.json` plus `default_wallpapers/{documents,thumbs/m}`).
//!
//! Usage:
//!   `appearancefetch sendcode`              env: `PHONE`
//!   `appearancefetch fetch <out>`           env: `PHONE CODE CODE_HASH PASSWORD`

use std::path::Path;
use std::process::ExitCode;

use rtd_fetchtools::client::{Tg, default_session_path, is_password_needed};
use rtd_fetchtools::creds::{TDESKTOP_API_HASH, TDESKTOP_API_ID, api_credentials, env_nonempty};
use rtd_fetchtools::seed::appearance::{
    Attribute, Catalog, ChatTheme, ColorOption, ColorSet, Document, Photo, ThemeSettings,
    Wallpaper, WallpaperSettings,
};
use rtd_fetchtools::{Result, fsutil, timefmt};
use rtd_tg::{
    AccountGetChatThemes, AccountGetWallPapers, AccountThemes, AccountWallPapers, AuthSentCode,
    BaseTheme, DocumentAttribute, HelpGetPeerColors, HelpGetPeerProfileColors, HelpPeerColorOption,
    HelpPeerColorSet, HelpPeerColors, InputFileLocation, Method, Object, PhotoSize, Theme,
    WallPaper, WallPaperSettings as TgWallpaperSettings,
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
    let Some(command) = args.get(1) else {
        eprintln!("usage: appearancefetch sendcode | appearancefetch fetch <out_dir> <carry_json>");
        return Ok(());
    };
    let session = default_session_path();
    let (api_id, api_hash) = api_credentials(TDESKTOP_API_ID, TDESKTOP_API_HASH);
    let mut tg = Tg::connect(api_id, &api_hash, &session)?;
    match command.as_str() {
        "sendcode" => send_code(&mut tg),
        "fetch" => {
            let Some(out) = args.get(2) else {
                return Err("fetch needs <out_dir>".into());
            };
            fetch(&mut tg, Path::new(out))
        }
        other => Err(format!("unknown cmd {other:?}").into()),
    }
}

/// `sendcode`: sends a login code and prints the code hash.
fn send_code(tg: &mut Tg) -> Result<()> {
    let phone = env_nonempty("PHONE").ok_or("PHONE env required")?;
    match tg.send_code(&phone)? {
        AuthSentCode::SentCode {
            phone_code_hash, ..
        }
        | AuthSentCode::SentCodePaymentRequired {
            phone_code_hash, ..
        } => {
            println!("CODE_HASH={phone_code_hash}");
            Ok(())
        }
        AuthSentCode::SentCodeSuccess { .. } => Err("sentCode = auth.sentCodeSuccess".into()),
    }
}

/// `fetch <out>`: signs in if needed, then snapshots wallpapers and themes.
fn fetch(tg: &mut Tg, out: &Path) -> Result<()> {
    let phone = env_nonempty("PHONE").ok_or("PHONE env required")?;
    let code = env_nonempty("CODE").unwrap_or_default();
    let code_hash = env_nonempty("CODE_HASH").unwrap_or_default();
    let password = env_nonempty("PASSWORD").unwrap_or_default();

    if tg.is_authorized()? {
        println!("[already authorized]");
    } else {
        match tg.sign_in(&phone, &code_hash, &code) {
            Ok(_) => {}
            Err(error) if is_password_needed(&error) => {
                tg.check_password(password.as_bytes())?;
            }
            Err(error) => return Err(error),
        }
        tg.save()?;
        println!("[authorized]");
    }

    let docs_dir = out.join("default_wallpapers").join("documents");
    let thumbs_dir = out.join("default_wallpapers").join("thumbs").join("m");
    std::fs::create_dir_all(&docs_dir)?;
    std::fs::create_dir_all(&thumbs_dir)?;

    let wallpapers = fetch_wallpapers(tg, &docs_dir, &thumbs_dir)?;
    println!("[wallpapers] {}", wallpapers.len());
    let chat_themes = fetch_chat_themes(tg, &docs_dir, &thumbs_dir)?;
    println!("[chat_themes] {}", chat_themes.len());
    let peer_colors = fetch_colors(tg, false)?;
    let peer_profile_colors = fetch_colors(tg, true)?;
    println!(
        "[peer_colors] {} / [peer_profile_colors] {}",
        peer_colors.as_ref().map_or(0, Vec::len),
        peer_profile_colors.as_ref().map_or(0, Vec::len)
    );

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let catalog = Catalog {
        source: "upstream appearance snapshot".to_owned(),
        exported_at: timefmt::rfc3339_utc(now),
        server: "official".to_owned(),
        chat_themes,
        wallpapers,
        peer_colors,
        peer_profile_colors,
    };
    let json_path = out.join("default_appearance_seed.json");
    fsutil::write_file_atomic(&json_path, catalog.to_value().render(" ").as_bytes())?;
    println!(
        "wrote {} (wallpapers={} chat_themes={} peer_colors={} profile={})",
        json_path.display(),
        catalog.wallpapers.len(),
        catalog.chat_themes.len(),
        catalog.peer_colors.as_ref().map_or(0, Vec::len),
        catalog.peer_profile_colors.as_ref().map_or(0, Vec::len),
    );
    Ok(())
}

/// Converts one `Document`, downloading its body and `m` thumbnail.
fn convert_document(
    tg: &mut Tg,
    docs_dir: &Path,
    thumbs_dir: &Path,
    class: &rtd_tg::Document,
) -> Result<Document> {
    let rtd_tg::Document::Document {
        id,
        access_hash,
        file_reference,
        date,
        mime_type,
        size,
        dc_id,
        thumbs,
        attributes,
        ..
    } = class
    else {
        return Ok(Document::default());
    };
    if *id == 0 {
        return Ok(Document::default());
    }
    let data = download(tg, *id, *access_hash, file_reference, "", *size, *dc_id)?;
    let name = format!("{id}.dat");
    std::fs::write(docs_dir.join(&name), &data)?;
    let mut out = Document {
        id: *id,
        access_hash: *access_hash,
        date: *date,
        mime_type: mime_type.clone(),
        size: *size,
        dc_id: *dc_id,
        path: format!("default_wallpapers/documents/{name}"),
        sha256: fsutil::sha256_hex(&data),
        attributes: Vec::new(),
        thumbs: Vec::new(),
    };
    for attribute in attributes {
        match attribute {
            DocumentAttribute::DocumentAttributeImageSize { w, h } => {
                out.attributes.push(Attribute {
                    kind: "image_size".to_owned(),
                    w: *w,
                    h: *h,
                    ..Attribute::default()
                })
            }
            DocumentAttribute::DocumentAttributeFilename { file_name } => {
                out.attributes.push(Attribute {
                    kind: "filename".to_owned(),
                    file_name: file_name.clone(),
                    ..Attribute::default()
                });
            }
            _ => {}
        }
    }
    if let Some(thumbs) = thumbs {
        for thumb in thumbs {
            let PhotoSize::PhotoSize {
                type_: thumb_type,
                w,
                h,
                size: thumb_size,
            } = thumb
            else {
                continue;
            };
            if thumb_type != "m" {
                continue;
            }
            let tdata = download(
                tg,
                *id,
                *access_hash,
                file_reference,
                "m",
                i64::from(*thumb_size),
                *dc_id,
            )?;
            std::fs::write(thumbs_dir.join(&name), &tdata)?;
            out.thumbs.push(Photo {
                kind: "size".to_owned(),
                type_: "m".to_owned(),
                w: *w,
                h: *h,
                size: *thumb_size,
                path: format!("default_wallpapers/thumbs/m/{name}"),
                sha256: fsutil::sha256_hex(&tdata),
            });
            break;
        }
    }
    Ok(out)
}

/// Downloads a document body or thumbnail.
fn download(
    tg: &mut Tg,
    id: i64,
    access_hash: i64,
    file_reference: &[u8],
    thumb_size: &str,
    size: i64,
    dc_id: i32,
) -> Result<Vec<u8>> {
    let location = InputFileLocation::InputDocumentFileLocation {
        id,
        access_hash,
        file_reference: file_reference.to_vec(),
        thumb_size: thumb_size.to_owned(),
    };
    tg.download(location, usize::try_from(size).unwrap_or(0), dc_id)
}

/// Converts one wallpaper settings block.
fn convert_settings(settings: &TgWallpaperSettings) -> WallpaperSettings {
    WallpaperSettings {
        blur: settings.blur,
        motion: settings.motion,
        background_color: settings.background_color.unwrap_or(0),
        second_background_color: settings.second_background_color.unwrap_or(0),
        third_background_color: settings.third_background_color.unwrap_or(0),
        fourth_background_color: settings.fourth_background_color.unwrap_or(0),
        intensity: settings.intensity.unwrap_or(0),
        rotation: settings.rotation.unwrap_or(0),
    }
}

/// Converts one wallpaper.
fn convert_wallpaper(
    tg: &mut Tg,
    docs_dir: &Path,
    thumbs_dir: &Path,
    class: &WallPaper,
) -> Result<Wallpaper> {
    match class {
        WallPaper::WallPaper {
            id,
            access_hash,
            default,
            pattern,
            dark,
            slug,
            document,
            settings,
            ..
        } => {
            let document = convert_document(tg, docs_dir, thumbs_dir, document)?;
            let mut out = Wallpaper {
                id: *id,
                access_hash: *access_hash,
                type_: 0,
                default: *default,
                pattern: *pattern,
                dark: *dark,
                slug: slug.clone(),
                settings: WallpaperSettings::default(),
                document,
            };
            if let Some(settings) = settings {
                out.settings = convert_settings(settings);
            }
            Ok(out)
        }
        WallPaper::WallPaperNoFile {
            id,
            default,
            dark,
            settings,
            ..
        } => {
            let mut out = Wallpaper {
                id: *id,
                type_: 1,
                default: *default,
                dark: *dark,
                ..Wallpaper::default()
            };
            if let Some(settings) = settings {
                out.settings = convert_settings(settings);
            }
            Ok(out)
        }
    }
}

/// Fetches `account.getWallPapers`.
fn fetch_wallpapers(tg: &mut Tg, docs_dir: &Path, thumbs_dir: &Path) -> Result<Vec<Wallpaper>> {
    let request = Method::AccountGetWallPapers(Box::new(AccountGetWallPapers { hash: 0 }));
    let Object::AccountWallPapers(response) = tg.invoke(request)? else {
        return Ok(Vec::new());
    };
    let AccountWallPapers::WallPapers { wallpapers, .. } = &*response else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(wallpapers.len());
    for wallpaper in wallpapers {
        let converted = convert_wallpaper(tg, docs_dir, thumbs_dir, wallpaper)?;
        if converted.id != 0 {
            out.push(converted);
        }
    }
    Ok(out)
}

/// Fetches `account.getChatThemes`.
fn fetch_chat_themes(tg: &mut Tg, docs_dir: &Path, thumbs_dir: &Path) -> Result<Vec<ChatTheme>> {
    let request = Method::AccountGetChatThemes(Box::new(AccountGetChatThemes { hash: 0 }));
    let Object::AccountThemes(response) = tg.invoke(request)? else {
        return Ok(Vec::new());
    };
    let AccountThemes::Themes { themes, .. } = &*response else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(themes.len());
    for theme in themes {
        out.push(convert_chat_theme(tg, docs_dir, thumbs_dir, theme)?);
    }
    Ok(out)
}

/// Converts one chat theme.
fn convert_chat_theme(
    tg: &mut Tg,
    docs_dir: &Path,
    thumbs_dir: &Path,
    theme: &Theme,
) -> Result<ChatTheme> {
    let mut out = ChatTheme {
        id: theme.id,
        access_hash: theme.access_hash,
        slug: theme.slug.clone(),
        title: theme.title.clone(),
        emoticon: theme.emoticon.clone().unwrap_or_default(),
        default: theme.default,
        settings: Vec::new(),
    };
    if let Some(settings) = &theme.settings {
        for setting in settings {
            let mut converted = ThemeSettings {
                base_theme: base_theme_name(&setting.base_theme),
                accent_color: setting.accent_color,
                outbox_accent_color: setting.outbox_accent_color.unwrap_or(0),
                message_colors: setting.message_colors.clone(),
                wallpaper: Wallpaper::default(),
            };
            if let Some(wallpaper) = &setting.wallpaper {
                converted.wallpaper = convert_wallpaper(tg, docs_dir, thumbs_dir, wallpaper)?;
            }
            out.settings.push(converted);
        }
    }
    Ok(out)
}

/// Maps a base theme constructor to its name.
fn base_theme_name(theme: &BaseTheme) -> String {
    match theme {
        BaseTheme::BaseThemeClassic => "classic",
        BaseTheme::BaseThemeDay => "day",
        BaseTheme::BaseThemeNight => "night",
        BaseTheme::BaseThemeTinted => "tinted",
        BaseTheme::BaseThemeArctic => "arctic",
    }
    .to_owned()
}

/// Fetches peer colours (or peer profile colours).
fn fetch_colors(tg: &mut Tg, profile: bool) -> Result<Option<Vec<ColorOption>>> {
    let request = if profile {
        Method::HelpGetPeerProfileColors(Box::new(HelpGetPeerProfileColors { hash: 0 }))
    } else {
        Method::HelpGetPeerColors(Box::new(HelpGetPeerColors { hash: 0 }))
    };
    let Ok(Object::HelpPeerColors(response)) = tg.invoke(request) else {
        return Ok(None);
    };
    let HelpPeerColors::PeerColors { colors, .. } = &*response else {
        return Ok(None);
    };
    Ok(Some(colors.iter().map(convert_color_option).collect()))
}

/// Converts one peer colour option.
fn convert_color_option(option: &HelpPeerColorOption) -> ColorOption {
    ColorOption {
        id: option.color_id,
        hidden: option.hidden,
        channel_min_level: option.channel_min_level.unwrap_or(0),
        group_min_level: option.group_min_level.unwrap_or(0),
        colors: option.colors.as_deref().map(convert_color_set),
        dark_colors: option.dark_colors.as_deref().map(convert_color_set),
    }
}

/// Converts one peer colour set.
fn convert_color_set(set: &HelpPeerColorSet) -> ColorSet {
    match set {
        HelpPeerColorSet::PeerColorSet { colors } => ColorSet {
            predicate_name: "help.peerColorSet".to_owned(),
            colors: Some(colors.clone()),
            ..ColorSet::default()
        },
        HelpPeerColorSet::PeerColorProfileSet {
            palette_colors,
            bg_colors,
            story_colors,
        } => ColorSet {
            predicate_name: "help.peerColorProfileSet".to_owned(),
            palette_colors: Some(palette_colors.clone()),
            bg_colors: Some(bg_colors.clone()),
            story_colors: Some(story_colors.clone()),
            ..ColorSet::default()
        },
    }
}
