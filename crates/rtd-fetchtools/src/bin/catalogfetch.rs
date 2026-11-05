//! `catalogfetch` — pull reference data from official Telegram.
//!
//! Port of `cmd/catalogfetch`. Pulls country dialing codes and timezones
//! (login-free) plus emoji groups and keywords (login required, reusing the
//! session established by `appearancefetch`) and writes `internal/seed/catalog`
//! JSON.
//!
//! Usage: `SESSION=/tmp/appearance.session catalogfetch <out_dir> [langCode=en]`

use std::path::Path;
use std::process::ExitCode;

use rtd_fetchtools::client::Tg;
use rtd_fetchtools::creds::{self, TDESKTOP_API_HASH, TDESKTOP_API_ID};
use rtd_fetchtools::{Result, seed};
use rtd_tg::{
    HelpGetCountriesList, HelpGetTimezonesList, MessagesGetEmojiGroups, MessagesGetEmojiKeywords,
    Method, Object,
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
    let Some(out) = args.get(1) else {
        eprintln!("usage: SESSION=/tmp/appearance.session catalogfetch <out_dir> [langCode=en]");
        return Ok(());
    };
    let lang = args.get(2).cloned().unwrap_or_else(|| "en".to_owned());
    std::fs::create_dir_all(out)?;

    let (api_id, api_hash) = creds::api_credentials(TDESKTOP_API_ID, TDESKTOP_API_HASH);
    let session = rtd_fetchtools::client::default_session_path();
    let mut tg = Tg::connect(api_id, &api_hash, &session)?;
    if let Ok(authorized) = tg.is_authorized() {
        println!("[session {session} authorized={authorized}]");
    }

    fetch_countries(&mut tg, out, &lang);
    fetch_timezones(&mut tg, out);
    fetch_emoji_groups(&mut tg, out);
    fetch_emoji_keywords(&mut tg, out, &lang);
    Ok(())
}

fn fetch_countries(tg: &mut Tg, out: &str, lang: &str) {
    let request = Method::HelpGetCountriesList(Box::new(HelpGetCountriesList {
        lang_code: lang.to_owned(),
        hash: 0,
    }));
    match tg.invoke(request) {
        Ok(Object::HelpCountriesList(list)) => {
            let count = match &*list {
                rtd_tg::HelpCountriesList::CountriesList { countries, .. } => countries.len(),
                rtd_tg::HelpCountriesList::CountriesListNotModified => 0,
            };
            if let Err(error) = write_json(
                Path::new(out).join("countries.json"),
                &seed::catalog::countries(&list),
            ) {
                eprintln!("ERROR: {error}");
                return;
            }
            println!("[countries] {count}");
        }
        Ok(_) => {}
        Err(error) => eprintln!("skip countries: {error}"),
    }
}

fn fetch_timezones(tg: &mut Tg, out: &str) {
    let request = Method::HelpGetTimezonesList(Box::new(HelpGetTimezonesList { hash: 0 }));
    match tg.invoke(request) {
        Ok(Object::HelpTimezonesList(list)) => {
            let count = match &*list {
                rtd_tg::HelpTimezonesList::TimezonesList { timezones, .. } => timezones.len(),
                rtd_tg::HelpTimezonesList::TimezonesListNotModified => 0,
            };
            if let Err(error) = write_json(
                Path::new(out).join("timezones.json"),
                &seed::catalog::timezones(&list),
            ) {
                eprintln!("ERROR: {error}");
                return;
            }
            println!("[timezones] {count}");
        }
        Ok(_) => {}
        Err(error) => eprintln!("skip timezones: {error}"),
    }
}

fn fetch_emoji_groups(tg: &mut Tg, out: &str) {
    let request = Method::MessagesGetEmojiGroups(Box::new(MessagesGetEmojiGroups { hash: 0 }));
    match tg.invoke(request) {
        Ok(Object::MessagesEmojiGroups(groups)) => {
            let count = match &*groups {
                rtd_tg::MessagesEmojiGroups::EmojiGroups { groups, .. } => groups.len(),
                rtd_tg::MessagesEmojiGroups::EmojiGroupsNotModified => 0,
            };
            let value = match &*groups {
                rtd_tg::MessagesEmojiGroups::EmojiGroups { groups, .. } => {
                    seed::catalog::emoji_groups(groups)
                }
                rtd_tg::MessagesEmojiGroups::EmojiGroupsNotModified => {
                    seed::catalog::emoji_groups(&[])
                }
            };
            if let Err(error) = write_json(Path::new(out).join("emoji_groups.json"), &value) {
                eprintln!("ERROR: {error}");
                return;
            }
            println!("[emoji_groups] {count}");
        }
        Ok(_) => {}
        Err(error) => eprintln!("skip emoji_groups: {error}"),
    }
}

fn fetch_emoji_keywords(tg: &mut Tg, out: &str, lang: &str) {
    let request = Method::MessagesGetEmojiKeywords(Box::new(MessagesGetEmojiKeywords {
        lang_code: lang.to_owned(),
    }));
    match tg.invoke(request) {
        Ok(Object::EmojiKeywordsDifference(difference)) => {
            let value = seed::catalog::emoji_keywords(&difference);
            let name = if difference.lang_code.is_empty() {
                format!("emoji_keywords_{lang}.json")
            } else {
                format!("emoji_keywords_{}.json", difference.lang_code)
            };
            if let Err(error) = write_json(Path::new(out).join(name), &value) {
                eprintln!("ERROR: {error}");
                return;
            }
            println!(
                "[emoji_keywords {}] {} (v{})",
                difference.lang_code,
                difference.keywords.len(),
                difference.version
            );
        }
        Ok(_) => {}
        Err(error) => eprintln!("skip emoji_keywords: {error}"),
    }
}

fn write_json(path: std::path::PathBuf, value: &rtd_fetchtools::gojson::Value) -> Result<()> {
    let body = seed::json_file(value, "  ");
    std::fs::write(path, body)?;
    Ok(())
}
