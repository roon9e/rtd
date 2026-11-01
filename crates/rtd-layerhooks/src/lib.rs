//! Hand-written per-layer conversion hooks for the multi-layer sidecar.
//!
//! The generated `rtd-tlprofile` sidecar encodes and decodes every value in an
//! exact layer profile. Mechanical differences (field order, subset, presence)
//! are generated; the differences that are *not* mechanical — the conversions
//! reviewed in `schema/layers/policy.json` — are implemented here and called by
//! the generated code via the hook-signature registry.
//!
//! Each hook family lives in its own module. A hook converts a value between the
//! canonical layer and a profile in one direction; the `_encode` function maps
//! canonical to profile and `_decode` maps profile to canonical. Conversions
//! fail closed: a value that cannot be represented exactly is an error, never a
//! silent lossy projection.
#![warn(missing_docs)]

pub mod error;
pub mod text;

pub use error::{HookError, Result};

/// The hook names implemented in this crate, for the generation-time coverage
/// gate. Every present-in-both adapter obligation must name one of these, and
/// every name here must be referenced by an obligation.
pub const IMPLEMENTED_HOOKS: &[&str] = &[
    "layerBotVerificationText",
    "layerBotVerifierSettingsText",
    "layerBotsSetCustomVerificationText",
];

#[cfg(test)]
mod tests {
    use super::IMPLEMENTED_HOOKS;

    /// The hand-written implementations and the shared registry must agree, so a
    /// newly implemented hook cannot be forgotten in either place.
    #[test]
    fn implemented_hooks_match_the_registry() {
        let mut declared: Vec<&str> = IMPLEMENTED_HOOKS.to_vec();
        declared.sort_unstable();
        let mut registered = rtd_layerhooks_registry::implemented_names();
        registered.sort_unstable();
        assert_eq!(declared, registered);
    }
}
