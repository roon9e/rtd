#![no_main]

//! Fuzzes the multi-layer admission path with arbitrary bytes.
//!
//! Admission must fail closed (returning an error) rather than panicking on a
//! malformed request.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let profile =
        rtd_tlprofile::resolve_profile(rtd_tlprofile::CANONICAL_LAYER).expect("canonical layer");
    let _ = rtd_tlprofile::admit_default(profile, data);
});
