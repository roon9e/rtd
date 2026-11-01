//! Cross-checks against vectors emitted by the reference Go implementation
//! (`td`). The vectors are produced by `interop/vectors` (see task 8.1) and
//! committed at `interop/vectors.json`.

use rtd_proto::{Abridged, Codec, Intermediate, MessageType, new_message_id};
use serde_json::Value;

const VECTORS: &str = include_str!("../../../interop/vectors.json");

fn vectors() -> Vec<Value> {
    let document: Value = serde_json::from_str(VECTORS).expect("valid vectors json");
    document["vectors"]
        .as_array()
        .expect("vectors array")
        .clone()
}

fn entry<'a>(vectors: &'a [Value], name: &str) -> &'a Value {
    vectors
        .iter()
        .find(|v| v["name"] == name)
        .unwrap_or_else(|| panic!("missing vector {name}"))
}

fn hex_bytes(vector: &Value, key: &str) -> Vec<u8> {
    hex::decode(vector[key].as_str().expect("hex field")).expect("valid hex")
}

#[test]
fn message_ids_match_reference() {
    let vectors = vectors();
    let expected = entry(&vectors, "message_id");
    let now_nanos = 1_700_000_000_123_456_789i64;

    let from_client = new_message_id(now_nanos, MessageType::FromClient);
    let from_server = new_message_id(now_nanos, MessageType::FromServer);
    let response = new_message_id(now_nanos, MessageType::ServerResponse);

    assert_eq!(from_client.as_i64(), expected["i"].as_i64().expect("i"));
    assert_eq!(from_server.as_i64(), expected["i2"].as_i64().expect("i2"));
    assert_eq!(response.as_i64(), expected["i3"].as_i64().expect("i3"));
}

#[test]
fn abridged_frame_matches_reference() {
    let vectors = vectors();
    let expected = hex_bytes(entry(&vectors, "abridged_frame"), "a");
    let mut frame = Vec::new();
    Abridged
        .write_message(&mut frame, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("write abridged frame");
    assert_eq!(frame, expected);
}

#[test]
fn intermediate_frame_matches_reference() {
    let vectors = vectors();
    let expected = hex_bytes(entry(&vectors, "intermediate_frame"), "a");
    let mut frame = Vec::new();
    Intermediate
        .write_message(&mut frame, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("write intermediate frame");
    assert_eq!(frame, expected);
}

/// Proves the harness actually detects drift: a deliberately altered expected
/// value must fail the comparison.
#[test]
fn altered_expected_value_is_detected() {
    let vectors = vectors();
    let mut expected = hex_bytes(entry(&vectors, "intermediate_frame"), "a");
    let mut frame = Vec::new();
    Intermediate
        .write_message(&mut frame, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("write intermediate frame");
    assert_eq!(frame, expected);
    expected[0] ^= 0xff;
    assert_ne!(
        frame, expected,
        "the comparison must detect a changed vector"
    );
}
