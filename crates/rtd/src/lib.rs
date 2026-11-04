//! Facade crate re-exporting the rtd public surface.
//!
//! Depend on `rtd` to get the whole client library and the generated value
//! model behind a single crate: [`rtd_client`] is re-exported at the root, and
//! the generated Telegram model ([`rtd_tg`]) plus the TL error type
//! ([`rtd_error`]) are available as modules. The WebRTC media backend is on by
//! default; disable it with `default-features = false`.
#![warn(missing_docs)]

pub use rtd_client::*;

/// The generated canonical Telegram value and method model.
pub use rtd_tg;

/// The Telegram TL error type.
pub use rtd_error;
