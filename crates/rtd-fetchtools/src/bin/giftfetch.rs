//! `giftfetch` — snapshot the official star-gift catalog.
//!
//! Port of `cmd/giftfetch`. Snapshots `payments.getStarGifts`, the current
//! `payments.getStarGiftUpgradeAttributes` pools, and every referenced document,
//! writing `catalog.tl`, `upgrade-attributes/<id>.tl`, `documents/`, `thumbs/`
//! and `manifest.json`. Read-only: it never imports data or copies the session.
//!
//! Usage:
//!   `SESSION=<path> giftfetch -out <directory>`
//!   `SESSION=<path> giftfetch -login-qr`

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rtd_client::qr::login_url;
use rtd_fetchtools::client::{Tg, is_password_needed};
use rtd_fetchtools::creds::{TDESKTOP_API_HASH, TDESKTOP_API_ID, env_nonempty};
use rtd_fetchtools::proxy;
use rtd_fetchtools::seed::gift::{
    Background, CatalogManifest, DocumentManifest, FileArtifact, GiftManifest, MissingThumb,
    Rarity, UpgradeAttributeSet, UpgradeBackdrop, UpgradeModel, UpgradePattern,
};
use rtd_fetchtools::{Result, fsutil};
use rtd_tg::{
    AuthExportLoginToken, AuthImportLoginToken, AuthLoginToken, Document as TgDocument,
    DocumentAttribute, InputFileLocation, Method, Object, PaymentsGetStarGiftUpgradeAttributes,
    PaymentsGetStarGifts, PaymentsStarGiftUpgradeAttributes, PaymentsStarGifts, PhotoSize,
    StarGift, StarGiftAttribute, StarGiftAttributeRarity, VideoSize,
};

const DEFAULT_MAX_DOC_BYTES: i64 = 16 << 20;
const DEFAULT_WORKERS: usize = 8;
const MAX_CATALOG_GIFTS: usize = 5000;
const MAX_UPGRADE_ATTRS: usize = 10000;
const MAX_DOCUMENTS: usize = 50000;
const MAX_WORKERS: usize = 128;

/// Regular gifts without a Telegram title, filled from the store's pattern names.
const REGULAR_GIFT_TITLES: [(i64, &str); 11] = [
    (5170145012310081615, "Heart"),
    (5170250947678437525, "Gift Box"),
    (5168103777563050263, "Rose"),
    (5170144170496491616, "Cake"),
    (5170314324215857265, "Bouquet"),
    (5170564780938756245, "Rocket"),
    (6028601630662853006, "Champagne"),
    (5168043875654172773, "Trophy"),
    (5170690322832818290, "Ring"),
    (5170521118301225164, "Diamond"),
    (5992502615223568003, "Bear"),
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

/// Parsed command-line options.
struct Options {
    out: String,
    max_document_bytes: i64,
    skip_thumbs: bool,
    workers: usize,
    reuse_metadata: bool,
    allowed_missing_thumbs: BTreeSet<String>,
    login_qr: bool,
}

fn run() -> Result<()> {
    let opts = parse_args()?;
    let session = env_nonempty("SESSION").ok_or("SESSION is required")?;
    let proxy = match env_nonempty("SOCKS5_PROXY") {
        Some(raw) => Some(proxy::parse(&raw)?),
        None => None,
    };

    if opts.login_qr {
        let mut tg = Tg::connect_proxied(TDESKTOP_API_ID, TDESKTOP_API_HASH, &session, 2, proxy)?;
        return login_qr(&mut tg);
    }

    if opts.out.trim().is_empty() {
        eprintln!("usage: SESSION=/path/to/session giftfetch -out <directory>");
        eprintln!("       SESSION=/path/to/session giftfetch -login-qr");
        return Ok(());
    }
    if opts.max_document_bytes <= 0 || opts.max_document_bytes > 256 << 20 {
        return Err("max-document-bytes must be in (0, 256 MiB]".into());
    }
    if opts.workers == 0 || opts.workers > MAX_WORKERS {
        return Err(format!("workers must be in [1, {MAX_WORKERS}]").into());
    }

    let mut tg = Tg::connect_proxied(TDESKTOP_API_ID, TDESKTOP_API_HASH, &session, 2, proxy)?;
    if !tg.is_authorized()? {
        return Err("SESSION is not authorized (run giftfetch -login-qr first)".into());
    }
    println!("[session authorized]");
    fetch_catalog(&mut tg, &opts, &session)
}

/// Parses the command-line arguments.
fn parse_args() -> Result<Options> {
    let mut opts = Options {
        out: String::new(),
        max_document_bytes: DEFAULT_MAX_DOC_BYTES,
        skip_thumbs: false,
        workers: DEFAULT_WORKERS,
        reuse_metadata: false,
        allowed_missing_thumbs: BTreeSet::new(),
        login_qr: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-out" => opts.out = args.next().unwrap_or_default(),
            "-max-document-bytes" => {
                opts.max_document_bytes = args.next().unwrap_or_default().parse().unwrap_or(0);
            }
            "-skip-thumbs" => opts.skip_thumbs = true,
            "-workers" => opts.workers = args.next().unwrap_or_default().parse().unwrap_or(0),
            "-reuse-metadata" => opts.reuse_metadata = true,
            "-allow-missing-thumb" => {
                let raw = args.next().unwrap_or_default();
                opts.allowed_missing_thumbs = parse_allowed_missing_thumbs(&raw)?;
            }
            "-login-qr" => opts.login_qr = true,
            // Terminal QR only: accept the flag and its value so scripts do not break.
            "-qr-png" => {
                let _ = args.next();
            }
            _ => {}
        }
    }
    Ok(opts)
}

/// Signs in through a QR login and persists the session.
fn login_qr(tg: &mut Tg) -> Result<()> {
    if tg.is_authorized()? {
        println!("[session already authorized]");
        return Ok(());
    }
    println!("[qr] scan in Telegram -> Settings -> Devices -> Link Desktop Device");
    let mut token = export_token(tg)?;
    render_qr(&login_url(&token.0));
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let request = Method::AuthImportLoginToken(Box::new(AuthImportLoginToken {
            token: token.0.clone(),
        }));
        match tg.invoke(request) {
            Ok(Object::AuthLoginToken(state)) => match *state {
                AuthLoginToken::LoginTokenSuccess { authorization } => {
                    report_authorized(&authorization);
                    tg.save()?;
                    return Ok(());
                }
                AuthLoginToken::LoginToken {
                    expires,
                    token: fresh,
                } => {
                    token = (fresh, expires);
                }
                AuthLoginToken::LoginTokenMigrateTo { .. } => {
                    return Err(
                        "QR login requires a datacenter migration that is unsupported".into(),
                    );
                }
            },
            Err(error) if is_password_needed(&error) => {
                println!("[2fa] two-step verification password is required");
                let password = resolve_password()?;
                tg.check_password(password.as_bytes())?;
                tg.save()?;
                println!("[authorized]");
                return Ok(());
            }
            Err(error) => {
                // An expired token must be re-exported; anything else aborts.
                let message = format!("{error}");
                if message.contains("TOKEN_EXPIRED") || message.contains("TOKEN_INVALID") {
                    token = export_token(tg)?;
                    render_qr(&login_url(&token.0));
                    continue;
                }
                return Err(error);
            }
            Ok(_) => {
                return Err("auth.importLoginToken returned an unexpected response".into());
            }
        }
    }
}

/// Exports a QR login token and returns `(token bytes, expires)`.
fn export_token(tg: &mut Tg) -> Result<(Vec<u8>, i32)> {
    let request = Method::AuthExportLoginToken(Box::new(AuthExportLoginToken {
        api_id: tg.api_id(),
        api_hash: tg.api_hash().to_owned(),
        except_ids: Vec::new(),
    }));
    match tg.invoke(request)? {
        Object::AuthLoginToken(state) => match *state {
            AuthLoginToken::LoginToken { token, expires } => Ok((token, expires)),
            _ => Err("auth.exportLoginToken did not return a fresh token".into()),
        },
        _ => Err("auth.exportLoginToken returned an unexpected response".into()),
    }
}

/// Reports the authorized account.
fn report_authorized(_authorization: &rtd_tg::AuthAuthorization) {
    println!("[authorized]");
}

/// Returns the 2FA cloud password from `TG_PASSWORD` or stdin.
fn resolve_password() -> Result<String> {
    if let Some(password) = env_nonempty("TG_PASSWORD") {
        return Ok(password);
    }
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let password = line.trim_end_matches(['\n', '\r']).to_owned();
    if password.is_empty() {
        return Err("2FA password required: set TG_PASSWORD, or pipe it on stdin".into());
    }
    Ok(password)
}

/// Renders the login URL as a half-block terminal QR code.
fn render_qr(url: &str) {
    let Ok(code) = qrcode::QrCode::with_error_correction_level(url.as_bytes(), qrcode::EcLevel::H)
    else {
        eprintln!("[qr] failed to encode login QR");
        return;
    };
    let size = code.width();
    let colors = code.to_colors();
    let black = |x: usize, y: usize| colors[y * size + x] == qrcode::types::Color::Dark;
    const QUIET: usize = 2;
    let dim = size + 2 * QUIET;
    let mut out = String::new();
    let mut y = 0;
    while y < dim {
        for x in 0..dim {
            let top = y >= QUIET
                && y - QUIET < size
                && x >= QUIET
                && x - QUIET < size
                && black(x - QUIET, y - QUIET);
            let bottom = y + 1 >= QUIET
                && y + 1 - QUIET < size
                && x >= QUIET
                && x - QUIET < size
                && black(x - QUIET, y + 1 - QUIET);
            out.push(match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        out.push('\n');
        y += 2;
    }
    print!("{out}");
    let _ = std::io::stdout().flush();
}

/// A document and the purposes that reference it.
struct Source {
    document: TgDocument,
    purposes: BTreeSet<String>,
}

/// Collects distinct documents referenced by the catalog.
#[derive(Default)]
struct Collector {
    documents: BTreeMap<i64, Source>,
}

impl Collector {
    /// Registers a document, returning its id.
    fn add(&mut self, class: &TgDocument, purpose: &str) -> Result<i64> {
        let TgDocument::Document {
            id,
            size,
            mime_type,
            attributes,
            ..
        } = class
        else {
            return Err(format!("{purpose} references an invalid document").into());
        };
        if *id == 0 {
            return Err(format!("{purpose} references an invalid document").into());
        }
        if !has_renderable_sticker_attribute(attributes) {
            return Err(format!(
                "{purpose} document {id} has neither sticker nor custom-emoji attribute"
            )
            .into());
        }
        match self.documents.get_mut(id) {
            None => {
                let mut purposes = BTreeSet::new();
                purposes.insert(purpose.to_owned());
                self.documents.insert(
                    *id,
                    Source {
                        document: class.clone(),
                        purposes,
                    },
                );
            }
            Some(existing) => {
                let TgDocument::Document {
                    size: existing_size,
                    mime_type: existing_mime,
                    ..
                } = &existing.document
                else {
                    unreachable!()
                };
                if existing_size != size || existing_mime != mime_type {
                    return Err(format!("document {id} has conflicting metadata").into());
                }
                existing.purposes.insert(purpose.to_owned());
            }
        }
        Ok(*id)
    }
}

/// Fetches the catalog and writes the export tree.
fn fetch_catalog(tg: &mut Tg, opts: &Options, session: &str) -> Result<()> {
    let out = PathBuf::from(&opts.out);
    std::fs::create_dir_all(&out)?;

    let (catalog, raw_artifact) = if opts.reuse_metadata {
        let (catalog, artifact) = read_catalog_tl(&out)?;
        println!("[metadata reused] path={}", artifact.path);
        (catalog, artifact)
    } else {
        let request = Method::PaymentsGetStarGifts(Box::new(PaymentsGetStarGifts { hash: 0 }));
        let Object::PaymentsStarGifts(response) = tg.invoke(request)? else {
            return Err("payments.getStarGifts returned an unexpected response".into());
        };
        let mut writer = rtd_bin::Writer::new();
        response.encode(&mut writer);
        let artifact = write_artifact(&out, "catalog.tl", "tl", "", &writer.into_vec())?;
        (*response, artifact)
    };

    let PaymentsStarGifts::StarGifts {
        hash,
        gifts,
        chats,
        users,
        ..
    } = &catalog
    else {
        return Err("payments.getStarGifts(hash=0) returned not-modified".into());
    };
    if gifts.len() > MAX_CATALOG_GIFTS {
        return Err(format!(
            "gift catalog has {} entries, limit is {MAX_CATALOG_GIFTS}",
            gifts.len()
        )
        .into());
    }
    let mut manifest = CatalogManifest {
        schema: 2,
        hash: *hash,
        raw_catalog: raw_artifact,
        gift_count: gifts.len() as i32,
        chat_count: chats.len() as i32,
        user_count: users.len() as i32,
        boundary_note: "payments.getStarGifts(hash=0) plus payments.getStarGiftUpgradeAttributes for every currently upgradeable base gift: complete current official attribute definitions and referenced documents, not deleted historical definitions or precomputed model-pattern-backdrop combinations".to_owned(),
        ..CatalogManifest::default()
    };

    let mut collector = Collector::default();
    for (index, class) in gifts.iter().enumerate() {
        let gift = collect_gift(index, class, &mut collector)?;
        manifest.gifts.push(gift);
    }

    let upgradeable = collect_upgradeable_gift_ids(gifts);
    manifest.upgradeable_gift_count = upgradeable.len() as i32;
    for gift_id in &upgradeable {
        let set =
            fetch_upgrade_attribute_set(tg, &out, *gift_id, &mut collector, opts.reuse_metadata)?;
        manifest.upgrade_attribute_count += set.attribute_count;
        manifest.upgrade_model_count += set.models.len() as i32;
        manifest.upgrade_pattern_count += set.patterns.len() as i32;
        manifest.upgrade_backdrop_count += set.backdrops.len() as i32;
        println!(
            "[upgrade attributes] gift_id={gift_id} models={} patterns={} backdrops={} raw={}",
            set.models.len(),
            set.patterns.len(),
            set.backdrops.len(),
            set.raw_attributes.path
        );
        manifest.upgrade_attribute_sets.push(set);
    }
    manifest.upgrade_attribute_set_count = manifest.upgrade_attribute_sets.len() as i32;
    if manifest.upgrade_attribute_set_count != manifest.upgradeable_gift_count {
        return Err(format!(
            "upgrade attribute set count mismatch: got {} want {}",
            manifest.upgrade_attribute_set_count, manifest.upgradeable_gift_count
        )
        .into());
    }
    if collector.documents.len() > MAX_DOCUMENTS {
        return Err(format!(
            "catalog and upgrade attributes reference {} documents, limit is {MAX_DOCUMENTS}",
            collector.documents.len()
        )
        .into());
    }

    download_documents(tg, opts, session, &out, &collector.documents, &mut manifest)?;

    let body = format!("{}\n", manifest.to_value().render("  "));
    fsutil::write_file_atomic(&out.join("manifest.json"), body.as_bytes())?;
    println!(
        "[complete] gifts={} attribute_sets={} attributes={} documents={} missing_thumbs={} bytes={} manifest={}",
        manifest.gifts.len(),
        manifest.upgrade_attribute_sets.len(),
        manifest.upgrade_attribute_count,
        manifest.documents.len(),
        manifest.missing_thumb_count,
        manifest.total_document_bytes,
        out.join("manifest.json").display()
    );
    Ok(())
}

/// Downloads the distinct documents, spreading work over `-workers` threads.
fn download_documents(
    tg: &mut Tg,
    opts: &Options,
    session: &str,
    out: &Path,
    documents: &BTreeMap<i64, Source>,
    manifest: &mut CatalogManifest,
) -> Result<()> {
    let _ = tg;
    let mut ids: Vec<i64> = documents.keys().copied().collect();
    ids.sort_unstable();
    println!(
        "[documents discovered] count={} workers={}",
        ids.len(),
        opts.workers
    );
    let results: Mutex<Vec<Option<DocumentManifest>>> = Mutex::new(vec![None; ids.len()]);
    let next = AtomicUsize::new(0);
    let proxy = match env_nonempty("SOCKS5_PROXY") {
        Some(raw) => Some(proxy::parse(&raw)?),
        None => None,
    };
    let workers = opts.workers.min(ids.len().max(1));
    std::thread::scope(|scope| -> Result<()> {
        let mut handles = Vec::new();
        for _ in 0..workers {
            let ids = &ids;
            let next = &next;
            let results = &results;
            let proxy = proxy.clone();
            handles.push(scope.spawn(move || -> Result<()> {
                let mut worker =
                    Tg::connect_proxied(TDESKTOP_API_ID, TDESKTOP_API_HASH, session, 2, proxy)?;
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= ids.len() {
                        break;
                    }
                    let id = ids[index];
                    let source = documents.get(&id).expect("document source");
                    let result = fetch_document(&mut worker, out, source, opts)?;
                    let mut guard = results.lock().expect("results");
                    if let Some(slot) = guard.get_mut(index) {
                        *slot = Some(result);
                    }
                }
                Ok(())
            }));
        }
        for handle in handles {
            match handle.join() {
                Ok(result) => result?,
                Err(_) => return Err("download worker panicked".into()),
            }
        }
        Ok(())
    })?;

    let mut results = results.into_inner().expect("results");
    for entry in results.iter_mut() {
        let Some(document) = entry.take() else {
            continue;
        };
        manifest.total_document_bytes += document.file.size;
        for thumb in &document.thumbs {
            manifest.total_document_bytes += thumb.size;
        }
        manifest.missing_thumb_count += document.missing_thumbs.len() as i32;
        manifest.documents.push(document);
    }
    Ok(())
}

/// Returns the ids of gifts that currently have upgrade attributes.
fn collect_upgradeable_gift_ids(gifts: &[StarGift]) -> Vec<i64> {
    let mut ids = Vec::new();
    for class in gifts {
        let StarGift::StarGift {
            id,
            upgrade_stars,
            upgrade_variants,
            ..
        } = class
        else {
            continue;
        };
        if *id == 0 {
            continue;
        }
        if upgrade_stars.unwrap_or(0) > 0 || upgrade_variants.unwrap_or(0) > 0 {
            ids.push(*id);
        }
    }
    ids
}

/// Builds the manifest entry for one catalog gift.
fn collect_gift(index: usize, class: &StarGift, collector: &mut Collector) -> Result<GiftManifest> {
    match class {
        StarGift::StarGift {
            id,
            sticker,
            stars,
            convert_stars,
            upgrade_stars,
            resell_min_stars,
            limited,
            sold_out,
            birthday,
            require_premium,
            limited_per_user,
            peer_color_available,
            auction,
            availability_remains,
            availability_total,
            availability_resale,
            per_user_total,
            per_user_remains,
            first_sale_date,
            last_sale_date,
            locked_until_date,
            auction_slug,
            gifts_per_round,
            auction_start_date,
            upgrade_variants,
            title,
            background,
            ..
        } => {
            let document_id = collector.add(sticker, &format!("gift:{id}:sticker"))?;
            let mut gift = GiftManifest {
                index: index as i32,
                kind: "regular".to_owned(),
                id: *id,
                stars: *stars,
                convert_stars: *convert_stars,
                upgrade_stars: upgrade_stars.unwrap_or(0),
                resell_min_stars: resell_min_stars.unwrap_or(0),
                limited: *limited,
                sold_out: *sold_out,
                birthday: *birthday,
                require_premium: *require_premium,
                limited_per_user: *limited_per_user,
                peer_color_available: *peer_color_available,
                auction: *auction,
                availability_remains: availability_remains.unwrap_or(0),
                availability_total: availability_total.unwrap_or(0),
                availability_resale: availability_resale.unwrap_or(0),
                per_user_total: per_user_total.unwrap_or(0),
                per_user_remains: per_user_remains.unwrap_or(0),
                first_sale_date: first_sale_date.unwrap_or(0),
                last_sale_date: last_sale_date.unwrap_or(0),
                locked_until_date: locked_until_date.unwrap_or(0),
                auction_slug: auction_slug.clone().unwrap_or_default(),
                gifts_per_round: gifts_per_round.unwrap_or(0),
                auction_start_date: auction_start_date.unwrap_or(0),
                upgrade_variants: upgrade_variants.unwrap_or(0),
                title: gift_title(*id, title.as_deref()),
                document_ids: vec![document_id],
                ..GiftManifest::default()
            };
            if let Some(background) = background {
                gift.background = Some(Background {
                    center_color: background.center_color,
                    edge_color: background.edge_color,
                    text_color: background.text_color,
                });
            }
            Ok(gift)
        }
        StarGift::StarGiftUnique {
            id,
            gift_id,
            title,
            slug,
            num,
            require_premium,
            availability_issued,
            availability_total,
            attributes,
            ..
        } => {
            let mut gift = GiftManifest {
                index: index as i32,
                kind: "unique".to_owned(),
                id: *id,
                gift_id: *gift_id,
                title: title.clone(),
                slug: slug.clone(),
                number: *num,
                require_premium: *require_premium,
                availability_issued: *availability_issued,
                availability_total: *availability_total,
                ..GiftManifest::default()
            };
            for attribute in attributes {
                let (document, purpose) = match attribute {
                    StarGiftAttribute::StarGiftAttributeModel { document, name, .. } => {
                        (document, format!("unique:{id}:model:{name}"))
                    }
                    StarGiftAttribute::StarGiftAttributePattern { document, name, .. } => {
                        (document, format!("unique:{id}:pattern:{name}"))
                    }
                    _ => continue,
                };
                let document_id = collector.add(document, &purpose)?;
                gift.document_ids.push(document_id);
            }
            Ok(gift)
        }
    }
}

/// Returns the gift title, falling back to the store pattern name.
fn gift_title(id: i64, title: Option<&str>) -> String {
    if let Some(title) = title
        && !title.trim().is_empty()
    {
        return title.to_owned();
    }
    REGULAR_GIFT_TITLES
        .iter()
        .find(|(gift_id, _)| *gift_id == id)
        .map(|(_, name)| (*name).to_owned())
        .unwrap_or_default()
}

/// Fetches (or reuses) one gift's upgrade attributes.
fn fetch_upgrade_attribute_set(
    tg: &mut Tg,
    root: &Path,
    gift_id: i64,
    collector: &mut Collector,
    reuse_metadata: bool,
) -> Result<UpgradeAttributeSet> {
    let rel = format!("upgrade-attributes/{gift_id}.tl");
    let (result, artifact) = if reuse_metadata {
        read_upgrade_tl(root, &rel)?
    } else {
        let request = Method::PaymentsGetStarGiftUpgradeAttributes(Box::new(
            PaymentsGetStarGiftUpgradeAttributes { gift_id },
        ));
        let Object::PaymentsStarGiftUpgradeAttributes(response) = tg.invoke(request)? else {
            return Err(format!(
                "payments.getStarGiftUpgradeAttributes(gift_id={gift_id}) returned an unexpected response"
            )
            .into());
        };
        let mut writer = rtd_bin::Writer::new();
        response.encode(&mut writer);
        let artifact = write_artifact(root, &rel, "tl", "", &writer.into_vec())?;
        (*response, artifact)
    };
    if result.attributes.is_empty() {
        return Err(format!(
            "payments.getStarGiftUpgradeAttributes(gift_id={gift_id}) returned no attributes"
        )
        .into());
    }
    if result.attributes.len() > MAX_UPGRADE_ATTRS {
        return Err(format!(
            "gift {gift_id} has {} upgrade attributes, limit is {MAX_UPGRADE_ATTRS}",
            result.attributes.len()
        )
        .into());
    }
    collect_upgrade_attributes(gift_id, &result, artifact, collector)
}

/// Converts one upgrade-attribute response.
fn collect_upgrade_attributes(
    gift_id: i64,
    result: &PaymentsStarGiftUpgradeAttributes,
    raw_artifact: FileArtifact,
    collector: &mut Collector,
) -> Result<UpgradeAttributeSet> {
    let mut set = UpgradeAttributeSet {
        gift_id,
        raw_attributes: raw_artifact,
        attribute_count: result.attributes.len() as i32,
        ..UpgradeAttributeSet::default()
    };
    let mut document_ids = BTreeSet::new();
    for attribute in &result.attributes {
        match attribute {
            StarGiftAttribute::StarGiftAttributeModel {
                name,
                document,
                crafted,
                rarity,
                ..
            } => {
                let rarity = collect_rarity(rarity)?;
                let document_id =
                    collector.add(document, &format!("gift:{gift_id}:upgrade-model:{name}"))?;
                set.models.push(UpgradeModel {
                    name: name.clone(),
                    document_id,
                    crafted: *crafted,
                    rarity,
                });
                document_ids.insert(document_id);
            }
            StarGiftAttribute::StarGiftAttributePattern {
                name,
                document,
                rarity,
            } => {
                let rarity = collect_rarity(rarity)?;
                let document_id =
                    collector.add(document, &format!("gift:{gift_id}:upgrade-pattern:{name}"))?;
                set.patterns.push(UpgradePattern {
                    name: name.clone(),
                    document_id,
                    rarity,
                });
                document_ids.insert(document_id);
            }
            StarGiftAttribute::StarGiftAttributeBackdrop {
                name,
                backdrop_id,
                center_color,
                edge_color,
                pattern_color,
                text_color,
                rarity,
            } => {
                let rarity = collect_rarity(rarity)?;
                set.backdrops.push(UpgradeBackdrop {
                    name: name.clone(),
                    backdrop_id: *backdrop_id,
                    center_color: *center_color,
                    edge_color: *edge_color,
                    pattern_color: *pattern_color,
                    text_color: *text_color,
                    rarity,
                });
            }
            StarGiftAttribute::StarGiftAttributeOriginalDetails { .. } => {
                return Err(format!("gift {gift_id} has an unsupported upgrade attribute").into());
            }
        }
    }
    if set.models.is_empty() || set.patterns.is_empty() || set.backdrops.is_empty() {
        return Err(format!(
            "gift {gift_id} incomplete upgrade attributes: models={} patterns={} backdrops={}",
            set.models.len(),
            set.patterns.len(),
            set.backdrops.len()
        )
        .into());
    }
    let parsed = set.models.len() + set.patterns.len() + set.backdrops.len();
    if parsed != result.attributes.len() {
        return Err(format!(
            "gift {gift_id} parsed {parsed} of {} upgrade attributes",
            result.attributes.len()
        )
        .into());
    }
    set.document_ids = document_ids.into_iter().collect();
    Ok(set)
}

/// Converts a rarity constructor.
fn collect_rarity(class: &StarGiftAttributeRarity) -> Result<Rarity> {
    let (kind, permille) = match class {
        StarGiftAttributeRarity::StarGiftAttributeRarity { permille } => {
            ("permille", Some(*permille))
        }
        StarGiftAttributeRarity::StarGiftAttributeRarityUncommon => ("uncommon", None),
        StarGiftAttributeRarity::StarGiftAttributeRarityRare => ("rare", None),
        StarGiftAttributeRarity::StarGiftAttributeRarityEpic => ("epic", None),
        StarGiftAttributeRarity::StarGiftAttributeRarityLegendary => ("legendary", None),
    };
    Ok(Rarity {
        kind: kind.to_owned(),
        constructor_id: format!("0x{:08x}", class.type_id()),
        permille,
    })
}

/// Downloads one document and its thumbnails.
fn fetch_document(
    tg: &mut Tg,
    root: &Path,
    source: &Source,
    opts: &Options,
) -> Result<DocumentManifest> {
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
    } = &source.document
    else {
        return Err("invalid document".into());
    };
    let max = opts.max_document_bytes;
    if *size <= 0 || *size > max {
        return Err(format!("document {id} size {size} is outside (0, {max}]").into());
    }
    let (file_name, sticker_alt) = document_names(attributes);
    let ext = document_extension(&file_name, mime_type);
    let rel = format!("documents/{id}{ext}");
    let data = match existing_artifact(root, &rel, *size, max)? {
        Some(data) => data,
        None => {
            println!(
                "[network fetch] document={id} resource=document expected_size={size} part_size={}",
                part_size(*size)
            );
            let location = InputFileLocation::InputDocumentFileLocation {
                id: *id,
                access_hash: *access_hash,
                file_reference: file_reference.clone(),
                thumb_size: String::new(),
            };
            download_retry(tg, location, *size, *dc_id, max)?
        }
    };
    if data.len() as i64 != *size {
        return Err(format!(
            "document {id} size mismatch: got {} want {size}",
            data.len()
        )
        .into());
    }
    let file = write_artifact(root, &rel, "document", "", &data)?;
    let purposes: Vec<String> = source.purposes.iter().cloned().collect();
    let mut document = DocumentManifest {
        id: *id,
        date: *date,
        dc_id: *dc_id,
        mime_type: mime_type.clone(),
        expected_size: *size,
        file_name,
        sticker_alt,
        purposes,
        file,
        ..DocumentManifest::default()
    };
    if opts.skip_thumbs {
        return Ok(document);
    }
    let (thumbs_artifacts, missing) = fetch_thumbs(
        tg,
        root,
        &source.document,
        max,
        &opts.allowed_missing_thumbs,
    )?;
    document.thumbs = thumbs_artifacts;
    document.missing_thumbs = missing;
    Ok(document)
}

/// Fetches a document's thumbnails into `thumbs/`.
fn fetch_thumbs(
    tg: &mut Tg,
    root: &Path,
    class: &TgDocument,
    max: i64,
    allowed_missing: &BTreeSet<String>,
) -> Result<(Vec<FileArtifact>, Vec<MissingThumb>)> {
    let TgDocument::Document {
        id,
        access_hash,
        file_reference,
        dc_id,
        thumbs,
        video_thumbs,
        ..
    } = class
    else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut seen = BTreeSet::new();
    let mut artifacts = Vec::new();
    let mut missing = Vec::new();
    if let Some(thumbs) = thumbs {
        for thumb in thumbs {
            let thumb_type = photo_type(thumb);
            if thumb_type.is_empty() {
                continue;
            }
            let key = format!("photo:{thumb_type}");
            if !seen.insert(key) {
                continue;
            }
            let mut expected = 0i64;
            let mut download = false;
            let data_opt: Option<Vec<u8>>;
            match thumb {
                PhotoSize::PhotoCachedSize { bytes, .. }
                | PhotoSize::PhotoStrippedSize { bytes, .. }
                | PhotoSize::PhotoPathSize { bytes, .. } => {
                    data_opt = Some(bytes.clone());
                }
                PhotoSize::PhotoSize { size, .. } => {
                    expected = i64::from(*size);
                    if expected <= 0 || expected > max {
                        return Err(format!(
                            "document {id} photo thumb {thumb_type} size {expected} is outside (0, {max}]"
                        )
                        .into());
                    }
                    let rel = format!("thumbs/{id}-photo-{}.bin", safe_part(thumb_type));
                    download = true;
                    data_opt = existing_artifact(root, &rel, expected, max)?;
                }
                PhotoSize::PhotoSizeProgressive { sizes, .. } => {
                    let Some(last) = sizes.last() else {
                        return Err(format!(
                            "document {id} progressive photo thumb {thumb_type} has no sizes"
                        )
                        .into());
                    };
                    expected = i64::from(*last);
                    if expected <= 0 || expected > max {
                        return Err(format!(
                            "document {id} progressive photo thumb {thumb_type} size {expected} is outside (0, {max}]"
                        )
                        .into());
                    }
                    let rel = format!("thumbs/{id}-photo-{}.bin", safe_part(thumb_type));
                    download = true;
                    data_opt = existing_artifact(root, &rel, expected, max)?;
                }
                _ => continue,
            }
            let data = match data_opt {
                Some(data) => data,
                None if download => {
                    let location =
                        doc_location(*id, *access_hash, file_reference, *dc_id, thumb_type);
                    match download_retry(tg, location, expected, *dc_id, max) {
                        Ok(data) => data,
                        Err(error) => {
                            if missing_thumb_allowed(allowed_missing, *id, "photo", thumb_type) {
                                missing.push(MissingThumb {
                                    kind: "photo".to_owned(),
                                    type_: thumb_type.to_owned(),
                                    expected_size: expected,
                                    error: format!("{error}"),
                                });
                                println!(
                                    "[missing thumb] document={id} kind=photo type={thumb_type} expected_size={expected} error={error}"
                                );
                                continue;
                            }
                            return Err(error);
                        }
                    }
                }
                None => continue,
            };
            if data.is_empty() {
                continue;
            }
            if expected > 0 && data.len() as i64 != expected {
                return Err(format!(
                    "document {id} photo thumb {thumb_type} size mismatch: got {} want {expected}",
                    data.len()
                )
                .into());
            }
            let rel = format!("thumbs/{id}-photo-{}.bin", safe_part(thumb_type));
            artifacts.push(write_artifact(root, &rel, "photo", thumb_type, &data)?);
        }
    }
    if let Some(video_thumbs) = video_thumbs {
        for thumb in video_thumbs {
            let VideoSize::VideoSize { type_, size, .. } = thumb else {
                continue;
            };
            if type_.is_empty() {
                continue;
            }
            let key = format!("video:{type_}");
            if !seen.insert(key) {
                continue;
            }
            let expected = i64::from(*size);
            let rel = format!("thumbs/{id}-video-{}.bin", safe_part(type_));
            let data = match existing_artifact(root, &rel, expected, max)? {
                Some(data) => data,
                None => {
                    let location = doc_location(*id, *access_hash, file_reference, *dc_id, type_);
                    match download_retry(tg, location, expected, *dc_id, max) {
                        Ok(data) => data,
                        Err(error) => {
                            if missing_thumb_allowed(allowed_missing, *id, "video", type_) {
                                missing.push(MissingThumb {
                                    kind: "video".to_owned(),
                                    type_: type_.clone(),
                                    expected_size: expected,
                                    error: format!("{error}"),
                                });
                                println!(
                                    "[missing thumb] document={id} kind=video type={type_} expected_size={expected} error={error}"
                                );
                                continue;
                            }
                            return Err(error);
                        }
                    }
                }
            };
            artifacts.push(write_artifact(root, &rel, "video", type_, &data)?);
        }
    }
    artifacts.sort_by(|a, b| (&a.kind, &a.type_).cmp(&(&b.kind, &b.type_)));
    missing.sort_by(|a, b| (&a.kind, &a.type_).cmp(&(&b.kind, &b.type_)));
    Ok((artifacts, missing))
}

/// Returns a thumbnail's size type.
fn photo_type(thumb: &PhotoSize) -> &str {
    match thumb {
        PhotoSize::PhotoCachedSize { type_, .. }
        | PhotoSize::PhotoPathSize { type_, .. }
        | PhotoSize::PhotoSize { type_, .. }
        | PhotoSize::PhotoSizeEmpty { type_ }
        | PhotoSize::PhotoSizeProgressive { type_, .. }
        | PhotoSize::PhotoStrippedSize { type_, .. } => type_,
    }
}

/// Builds a document file location.
fn doc_location(
    id: i64,
    access_hash: i64,
    file_reference: &[u8],
    _dc_id: i32,
    thumb_size: &str,
) -> InputFileLocation {
    InputFileLocation::InputDocumentFileLocation {
        id,
        access_hash,
        file_reference: file_reference.to_vec(),
        thumb_size: thumb_size.to_owned(),
    }
}

/// Downloads with the Go retry/backoff policy.
fn download_retry(
    tg: &mut Tg,
    location: InputFileLocation,
    expected: i64,
    dc_id: i32,
    max: i64,
) -> Result<Vec<u8>> {
    let mut last = String::new();
    for attempt in 1..=6 {
        match tg.download(
            location.clone(),
            usize::try_from(expected).unwrap_or(0),
            dc_id,
        ) {
            Ok(data) => {
                if data.len() as i64 > max {
                    return Err(format!("download exceeds {max} bytes").into());
                }
                return Ok(data);
            }
            Err(error) => {
                last = format!("{error}");
                if !is_retryable(&last) {
                    return Err(error);
                }
                let backoff = Duration::from_secs(attempt * 2);
                println!("[retry {attempt}/6] {last} (waiting {backoff:?})");
                std::thread::sleep(backoff);
            }
        }
    }
    Err(format!("failed after 6 attempts: {last}").into())
}

/// Returns whether a download error is worth retrying.
fn is_retryable(message: &str) -> bool {
    message.contains("-503")
        || message.contains("Timeout")
        || message.contains("FLOOD_WAIT")
        || message.contains("context deadline exceeded")
        || message.contains("connection reset by peer")
}

/// Returns document names from the attributes.
fn document_names(attributes: &[DocumentAttribute]) -> (String, String) {
    let mut file_name = String::new();
    let mut alt = String::new();
    for attribute in attributes {
        match attribute {
            DocumentAttribute::DocumentAttributeFilename { file_name: name } => {
                file_name = Path::new(name)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            DocumentAttribute::DocumentAttributeSticker {
                alt: sticker_alt, ..
            }
            | DocumentAttribute::DocumentAttributeCustomEmoji {
                alt: sticker_alt, ..
            } => {
                alt = sticker_alt.clone();
            }
            _ => {}
        }
    }
    (file_name, alt)
}

/// Maps a file name and MIME type to an extension.
fn document_extension(file_name: &str, mime_type: &str) -> String {
    let ext = Path::new(file_name)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    match ext.as_str() {
        ".tgs" | ".webm" | ".mp4" | ".webp" | ".png" | ".jpg" | ".jpeg" => return ext,
        _ => {}
    }
    match mime_type.to_lowercase().as_str() {
        "application/x-tgsticker" | "application/gzip" => ".tgs".to_owned(),
        "video/webm" => ".webm".to_owned(),
        "video/mp4" => ".mp4".to_owned(),
        "image/webp" => ".webp".to_owned(),
        "image/png" => ".png".to_owned(),
        "image/jpeg" => ".jpg".to_owned(),
        _ => ".bin".to_owned(),
    }
}

/// Keeps only safe path characters, mirroring Go's `safePart`.
fn safe_part(value: &str) -> String {
    let filtered: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    if filtered.is_empty() {
        "unknown".to_owned()
    } else {
        filtered
    }
}

/// Returns whether the document has a sticker or custom-emoji attribute.
fn has_renderable_sticker_attribute(attributes: &[DocumentAttribute]) -> bool {
    attributes.iter().any(|attribute| {
        matches!(
            attribute,
            DocumentAttribute::DocumentAttributeSticker { .. }
                | DocumentAttribute::DocumentAttributeCustomEmoji { .. }
        )
    })
}

/// Reads an existing artifact when its size matches.
fn existing_artifact(
    root: &Path,
    relative: &str,
    expected: i64,
    max: i64,
) -> Result<Option<Vec<u8>>> {
    match std::fs::read(root.join(relative)) {
        Ok(data) => {
            let size = data.len() as i64;
            if size <= 0 || size > max || (expected >= 0 && size != expected) {
                Ok(None)
            } else {
                Ok(Some(data))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Writes an artifact and returns its manifest entry.
fn write_artifact(
    root: &Path,
    relative: &str,
    kind: &str,
    artifact_type: &str,
    data: &[u8],
) -> Result<FileArtifact> {
    fsutil::write_file_atomic(&root.join(relative), data)?;
    Ok(FileArtifact {
        kind: kind.to_owned(),
        type_: artifact_type.to_owned(),
        path: relative.replace('\\', "/"),
        size: data.len() as i64,
        sha256: fsutil::sha256_hex(data),
    })
}

/// Reads and decodes `catalog.tl`.
fn read_catalog_tl(root: &Path) -> Result<(PaymentsStarGifts, FileArtifact)> {
    let data = read_tl(root, "catalog.tl")?;
    let mut decoder = rtd_tg::Decoder::new(&data, rtd_tg::DecodeLimits::default());
    let catalog = PaymentsStarGifts::decode(&mut decoder)
        .map_err(|error| format!("decode catalog.tl: {error}"))?;
    ensure_consumed(&decoder, "catalog.tl")?;
    Ok((catalog, tl_artifact("catalog.tl", &data)))
}

/// Reads and decodes one `upgrade-attributes/<id>.tl`.
fn read_upgrade_tl(
    root: &Path,
    relative: &str,
) -> Result<(PaymentsStarGiftUpgradeAttributes, FileArtifact)> {
    let data = read_tl(root, relative)?;
    let mut decoder = rtd_tg::Decoder::new(&data, rtd_tg::DecodeLimits::default());
    let result = PaymentsStarGiftUpgradeAttributes::decode(&mut decoder)
        .map_err(|error| format!("decode {relative}: {error}"))?;
    ensure_consumed(&decoder, relative)?;
    Ok((result, tl_artifact(relative, &data)))
}

/// Reads a TL artifact file.
fn read_tl(root: &Path, relative: &str) -> Result<Vec<u8>> {
    let data = std::fs::read(root.join(relative))?;
    if data.is_empty() {
        return Err(format!("TL artifact {relative:?} is empty").into());
    }
    Ok(data)
}

/// Verifies a decoder consumed the whole buffer.
fn ensure_consumed(decoder: &rtd_tg::Decoder<'_>, relative: &str) -> Result<()> {
    if decoder.remaining() != 0 {
        return Err(format!(
            "TL artifact {relative:?} has {} trailing bytes",
            decoder.remaining()
        )
        .into());
    }
    Ok(())
}

/// Builds a TL artifact entry.
fn tl_artifact(relative: &str, data: &[u8]) -> FileArtifact {
    FileArtifact {
        kind: "tl".to_owned(),
        type_: String::new(),
        path: relative.replace('\\', "/"),
        size: data.len() as i64,
        sha256: fsutil::sha256_hex(data),
    }
}

/// Parses `document_id:photo|video:type` entries.
fn parse_allowed_missing_thumbs(raw: &str) -> Result<BTreeSet<String>> {
    let mut allowed = BTreeSet::new();
    for entry in raw.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let parts: Vec<&str> = entry.split(':').collect();
        if parts.len() != 3 {
            return Err(format!(
                "invalid allow-missing-thumb {entry:?}: want document_id:photo|video:type"
            )
            .into());
        }
        let document_id: i64 = parts[0]
            .parse()
            .map_err(|_| format!("invalid allow-missing-thumb document ID {:?}", parts[0]))?;
        if document_id <= 0 {
            return Err(format!("invalid allow-missing-thumb document ID {:?}", parts[0]).into());
        }
        let kind = parts[1];
        if kind != "photo" && kind != "video" {
            return Err(format!("invalid allow-missing-thumb kind {kind:?}").into());
        }
        let thumb_type = parts[2];
        if thumb_type.is_empty() || safe_part(thumb_type) != thumb_type {
            return Err(format!("invalid allow-missing-thumb type {thumb_type:?}").into());
        }
        allowed.insert(format!("{document_id}:{kind}:{thumb_type}"));
    }
    Ok(allowed)
}

/// Returns whether a missing thumbnail is explicitly allowed.
fn missing_thumb_allowed(
    allowed: &BTreeSet<String>,
    document_id: i64,
    kind: &str,
    thumb_type: &str,
) -> bool {
    allowed.contains(&format!("{document_id}:{kind}:{thumb_type}"))
}

/// Returns the client-compatible chunk ladder for a known size.
fn part_size(expected: i64) -> i64 {
    const UNIT: i64 = 4 << 10;
    const MAX: i64 = 512 << 10;
    if expected <= 0 || expected >= MAX {
        return MAX;
    }
    let mut size = UNIT;
    while size <= expected && size < MAX {
        size *= 2;
    }
    size
}

#[cfg(test)]
mod tests {
    use super::{document_extension, parse_allowed_missing_thumbs, part_size, safe_part};

    #[test]
    fn safe_part_filters_and_falls_back() {
        assert_eq!(safe_part("a_b-c9"), "a_b-c9");
        assert_eq!(safe_part("m/../x"), "mx");
        assert_eq!(safe_part("!!!"), "unknown");
    }

    #[test]
    fn document_extension_prefers_file_name_then_mime() {
        assert_eq!(document_extension("a.tgs", ""), ".tgs");
        assert_eq!(document_extension("pic.JPG", ""), ".jpg");
        assert_eq!(document_extension("", "application/x-tgsticker"), ".tgs");
        assert_eq!(document_extension("", "video/mp4"), ".mp4");
        assert_eq!(document_extension("", ""), ".bin");
    }

    #[test]
    fn part_size_uses_the_chunk_ladder() {
        assert_eq!(part_size(0), 512 << 10);
        assert_eq!(part_size(1000), 4 << 10);
        assert_eq!(part_size(5 << 10), 8 << 10);
        assert_eq!(part_size(512 << 10), 512 << 10);
    }

    #[test]
    fn parses_allowed_missing_thumbs() {
        let allowed = parse_allowed_missing_thumbs("5:photo:m, 7:video:v").unwrap();
        assert!(allowed.contains("5:photo:m"));
        assert!(allowed.contains("7:video:v"));
        assert!(parse_allowed_missing_thumbs("5:photo").is_err());
        assert!(parse_allowed_missing_thumbs("5:bogus:m").is_err());
        assert!(parse_allowed_missing_thumbs("0:photo:m").is_err());
    }
}
