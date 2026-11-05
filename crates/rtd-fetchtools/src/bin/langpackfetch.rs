//! `langpackfetch` — pull language packs from official Telegram.
//!
//! Port of `cmd/langpackfetch`. Uses the login-free `langpack.getLangPack`
//! public method and converts the response into the telesrv `.strings` seed
//! format plus an `official-language-packs.json` manifest.
//!
//! Usage:
//!   `langpackfetch languages [pack]`
//!   `langpackfetch all <out_dir> [pack...]`
//!   `langpackfetch <out_dir> <langCode> [pack...]`

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rtd_fetchtools::client::Tg;
use rtd_fetchtools::creds::{TDESKTOP_API_HASH, TDESKTOP_API_ID};
use rtd_fetchtools::seed::langpack::{LanguageManifest, Manifest, PackManifest};
use rtd_fetchtools::{Result, fsutil, seed};
use rtd_tg::{
    LangPackDifference, LangPackLanguage, LangpackGetLangPack, LangpackGetLanguages, Method, Object,
};

/// Packs pulled by `all` when none are named.
const OFFICIAL_PACKS: [&str; 7] = [
    "android",
    "android_x",
    "ios",
    "macos",
    "tdesktop",
    "weba",
    "webk",
];

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
        eprintln!(
            "usage:\n  langpackfetch languages [pack]\n  langpackfetch all <out_dir> [pack...]\n  langpackfetch <out_dir> <langCode> [pack...]"
        );
        return Ok(());
    };

    let (api_id, api_hash) =
        rtd_fetchtools::creds::api_credentials(TDESKTOP_API_ID, TDESKTOP_API_HASH);
    let mut tg = Tg::ephemeral(api_id, &api_hash, 2)?;

    if command == "languages" {
        let pack = args.get(2).map_or("android", String::as_str);
        return list_languages(&mut tg, pack);
    }
    if command == "all" {
        let Some(root) = args.get(2) else {
            return Err("all needs <out_dir>".into());
        };
        let packs: Vec<String> = if args.len() > 3 {
            args[3..].to_vec()
        } else {
            OFFICIAL_PACKS
                .iter()
                .map(|pack| (*pack).to_owned())
                .collect()
        };
        return fetch_all(&mut tg, Path::new(root), &packs);
    }

    let root = Path::new(command);
    let lang_code = args.get(2).cloned().unwrap_or_else(|| "en".to_owned());
    let packs: Vec<String> = if args.len() > 3 {
        args[3..].to_vec()
    } else {
        vec!["android".to_owned(), "ios".to_owned(), "macos".to_owned()]
    };
    for pack in &packs {
        match get_lang_pack(&mut tg, pack, &lang_code) {
            Ok(difference) => {
                write_pack(root, pack, &difference)?;
            }
            Err(error) => eprintln!("skip {pack}/{lang_code}: {error}"),
        }
    }
    Ok(())
}

/// `languages [pack]`: prints the pack's language list.
fn list_languages(tg: &mut Tg, pack: &str) -> Result<()> {
    let languages = get_languages(tg, pack)?;
    println!("pack={pack}, {} languages:", languages.len());
    for language in &languages {
        let base = language.base_lang_code.clone().unwrap_or_default();
        println!(
            "  {:<18} {:<26} official={:<5} beta={:<5} strings={:<6} translated={:<6} base={:?}",
            language.lang_code,
            language.name,
            language.official,
            language.beta,
            language.strings_count,
            language.translated_count,
            base,
        );
    }
    Ok(())
}

/// `all <out_dir> [pack...]`: pulls every official language of every pack.
fn fetch_all(tg: &mut Tg, root: &Path, packs: &[String]) -> Result<()> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut manifest = Manifest::new();
    for raw in packs {
        let pack = raw.trim().to_ascii_lowercase();
        if !pack_name_valid(&pack) {
            return Err(format!("invalid lang pack {pack:?}").into());
        }
        if !seen.insert(pack.clone()) {
            continue;
        }
        let mut languages = get_languages(tg, &pack)?;
        languages.sort_by(|left, right| left.lang_code.cmp(&right.lang_code));
        let mut pack_manifest = PackManifest {
            name: pack.clone(),
            languages: Vec::new(),
        };
        for language in &languages {
            if !language.official {
                continue;
            }
            let code = language.lang_code.trim().to_ascii_lowercase();
            if !lang_code_valid(&code) {
                return Err(format!(
                    "invalid language code {:?} returned for {pack:?}",
                    language.lang_code
                )
                .into());
            }
            let difference = get_lang_pack(tg, &pack, &code)?;
            let (path, sha256) = write_pack(root, &pack, &difference)?;
            let file = relative_slash(root, &path);
            pack_manifest.languages.push(LanguageManifest {
                lang_code: code,
                name: language.name.clone(),
                native_name: language.native_name.clone(),
                base_lang_code: language.base_lang_code.clone(),
                plural_code: language.plural_code.clone(),
                official: language.official,
                rtl: language.rtl,
                beta: language.beta,
                strings_count: language.strings_count,
                translated_count: language.translated_count,
                translations_url: language.translations_url.clone(),
                version: difference.version,
                file,
                sha256,
            });
        }
        println!(
            "pack {pack} complete: {} official languages",
            pack_manifest.languages.len()
        );
        manifest.packs.push(pack_manifest);
    }
    let body = seed::json_file(&manifest.to_value(), "  ");
    fsutil::write_file_atomic(&root.join("official-language-packs.json"), body.as_bytes())?;
    Ok(())
}

/// Fetches `langpack.getLanguages` and returns the language list.
fn get_languages(tg: &mut Tg, pack: &str) -> Result<Vec<LangPackLanguage>> {
    let request = Method::LangpackGetLanguages(Box::new(LangpackGetLanguages {
        lang_pack: pack.to_owned(),
    }));
    let items = tg.invoke_vector(request)?;
    let mut languages = Vec::with_capacity(items.len());
    for item in items {
        if let Object::LangPackLanguage(language) = item {
            languages.push(*language);
        }
    }
    Ok(languages)
}

/// Fetches one `langpack.getLangPack` difference.
fn get_lang_pack(tg: &mut Tg, pack: &str, lang_code: &str) -> Result<LangPackDifference> {
    let request = Method::LangpackGetLangPack(Box::new(LangpackGetLangPack {
        lang_pack: pack.to_owned(),
        lang_code: lang_code.to_owned(),
    }));
    match tg.invoke(request)? {
        Object::LangPackDifference(difference) => Ok(*difference),
        _ => Err(format!("getLangPack {pack}/{lang_code} did not return a difference").into()),
    }
}

/// Writes one pack's `.strings` file and returns its path and SHA-256.
fn write_pack(
    root: &Path,
    pack: &str,
    difference: &LangPackDifference,
) -> Result<(PathBuf, String)> {
    let pack = pack.trim().to_ascii_lowercase();
    if !pack_name_valid(&pack) {
        return Err(format!("invalid lang pack {pack:?}").into());
    }
    let (body, count) = seed::langpack::strings_body(difference);
    let lang_code = difference.lang_code.trim().to_ascii_lowercase();
    if lang_code.is_empty() {
        return Err(format!("empty language code returned for {pack:?}").into());
    }
    if !lang_code_valid(&lang_code) {
        return Err(format!(
            "invalid language code {:?} returned for {pack:?}",
            difference.lang_code
        )
        .into());
    }
    let name = format!("{pack}_{lang_code}_v{}.strings", difference.version);
    let path = root.join(&pack).join(name);
    fsutil::write_file_atomic(&path, body.as_bytes())?;
    let sha256 = fsutil::sha256_hex(body.as_bytes());
    println!(
        "wrote {} ({count} strings, version {})",
        path.display(),
        difference.version
    );
    Ok((path, sha256))
}

/// Returns `path` relative to `root` with forward slashes.
fn relative_slash(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => path.to_string_lossy().replace('\\', "/"),
    }
}

/// Validates a pack name against `^[a-z0-9][a-z0-9_-]{0,31}$`.
fn pack_name_valid(pack: &str) -> bool {
    let bytes = pack.as_bytes();
    if bytes.is_empty() || bytes.len() > 32 {
        return false;
    }
    let first = bytes[0];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes[1..].iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
    })
}

/// Validates a language code against `^[a-z0-9]{1,16}(?:[-_][a-z0-9]{1,16})*$`.
fn lang_code_valid(code: &str) -> bool {
    if code.is_empty() {
        return false;
    }
    let mut segment_len = 0;
    for byte in code.bytes() {
        if byte == b'-' || byte == b'_' {
            if segment_len == 0 {
                return false;
            }
            segment_len = 0;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            segment_len += 1;
            if segment_len > 16 {
                return false;
            }
        } else {
            return false;
        }
    }
    segment_len > 0
}
