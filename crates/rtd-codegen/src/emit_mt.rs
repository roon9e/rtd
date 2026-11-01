//! Emits the single non-layered MTProto service model (`rtd-mt`).
//!
//! `mt.tl` is not a layered schema and contains one definition (`message`)
//! without an explicit identifier, so it is parsed with the standard TL CRC32
//! fallback. It is emitted through the same model emitter as the canonical
//! layer, which also supplies the `Object` and bare-`Message` forms the service
//! envelope needs.

use std::path::Path;

use crate::model::{SourceRef, build_schema_relaxed};
use crate::parse::parse_schema_with_crc;

/// Emits the `rtd-mt` source from `schema/mt.tl`.
///
/// # Errors
///
/// Returns a message when the schema cannot be read, parsed or emitted.
pub(crate) fn emit(root: &Path) -> Result<String, String> {
    let path = root.join("schema/mt.tl");
    let bytes =
        std::fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8", path.display()))?;
    let raw =
        parse_schema_with_crc(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let source = SourceRef {
        layer: 0,
        repository: "https://core.telegram.org/mtproto".to_owned(),
        commit: String::new(),
        blob: String::new(),
        path: "schema/mt.tl".to_owned(),
        file: "mt.tl".to_owned(),
        sha256: crate::digest::sha256(text.as_bytes()),
    };
    let model = build_schema_relaxed(&raw, source).map_err(|error| error.0)?;
    crate::emit_tg::emit(&model).map_err(|error| error.0)
}
