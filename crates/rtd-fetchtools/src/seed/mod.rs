//! Per-tool seed value models.
//!
//! Each submodule turns the raw Telegram response values into the exact JSON or
//! text seed format `telesrv` consumes, so the Rust fetchers are drop-in
//! replacements for the Go ones.

pub mod catalog;
pub mod langpack;

/// Appends the trailing newline Go's `writeJSON` adds after `MarshalIndent`.
#[must_use]
pub fn json_file(value: &crate::gojson::Value, indent: &str) -> String {
    let mut out = value.render(indent);
    out.push('\n');
    out
}
