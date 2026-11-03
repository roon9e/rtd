#![no_main]

//! Fuzzes the container and gzip decoders with arbitrary bytes.
//!
//! Neither decoder may panic or allocate without bound, so this target asserts
//! only that decoding returns a result.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = rtd_proto::MessageContainer::from_bytes(data);
    let _ = rtd_proto::Gzip::decode(data);
});
