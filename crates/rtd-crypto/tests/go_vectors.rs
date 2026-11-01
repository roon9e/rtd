//! Cross-checks against vectors emitted by the reference Go implementation
//! (`td`). The vectors are produced by `interop/vectors` (see task 8.1) and
//! committed at `interop/vectors.json`.

use rtd_crypto::{Key, Side, keys, message_key};
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

fn hex_arr<const N: usize>(vector: &Value, key: &str) -> [u8; N] {
    let bytes = hex::decode(vector[key].as_str().expect("hex field")).expect("valid hex");
    bytes.try_into().expect("expected length")
}

fn sequential_key() -> Key {
    let mut bytes = [0u8; 256];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = i as u8;
    }
    Key::from_array(bytes)
}

fn padded_plaintext() -> Vec<u8> {
    let mut data = vec![b'a'; 76];
    data.extend_from_slice(&[0u8; 16]);
    data
}

#[test]
fn message_keys_match_reference() {
    let vectors = vectors();
    let expected = entry(&vectors, "kdf_v2_message_key");
    let key = sequential_key();
    let plaintext = padded_plaintext();

    let client = message_key(&key, &plaintext, Side::Client);
    let server = message_key(&key, &plaintext, Side::Server);

    assert_eq!(client.as_array(), &hex_arr::<16>(expected, "a"));
    assert_eq!(server.as_array(), &hex_arr::<16>(expected, "b"));
}

#[test]
fn derived_keys_match_reference() {
    let vectors = vectors();
    let expected = entry(&vectors, "kdf_v2_keys");
    let key = sequential_key();
    let msg_key = message_key(&key, &padded_plaintext(), Side::Client);

    let (client_key, client_iv) = keys(&key, &msg_key, Side::Client);
    let (server_key, _) = keys(&key, &msg_key, Side::Server);

    assert_eq!(client_key, hex_arr::<32>(expected, "a"));
    assert_eq!(client_iv, hex_arr::<32>(expected, "b"));
    assert_eq!(server_key, hex_arr::<32>(expected, "c"));
}

#[test]
fn auth_key_fingerprint_matches_reference() {
    let vectors = vectors();
    let expected = entry(&vectors, "auth_key_id");
    let key = sequential_key();
    assert_eq!(key.id(), hex_arr::<8>(expected, "a"));
    assert_eq!(key.aux_hash(), hex_arr::<8>(expected, "b"));
}

/// Proves the harness actually detects drift: a deliberately altered expected
/// value must fail the comparison.
#[test]
fn altered_expected_value_is_detected() {
    let vectors = vectors();
    let expected = entry(&vectors, "kdf_v2_message_key");
    let key = sequential_key();
    let actual = message_key(&key, &padded_plaintext(), Side::Client);

    let mut corrupted = hex_arr::<16>(expected, "a");
    assert_eq!(actual.as_array(), &corrupted);
    corrupted[0] ^= 0xff;
    assert_ne!(
        actual.as_array(),
        &corrupted,
        "the comparison must detect a changed vector"
    );
}
