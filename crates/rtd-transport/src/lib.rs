//! Transport connections, obfuscation and proxying.
#![warn(missing_docs)]
//!
//! This crate implements the MTProxy obfuscation layers: [`obfuscated2`], which
//! wraps a connection in an AES-CTR stream keyed by a 64-byte random init, and
//! [`faketls`], which frames that stream into TLS records whose handshake
//! authenticates the proxy secret with HMAC-SHA256.
//!
//! Both layers implement [`std::io::Read`] and [`std::io::Write`] so they can
//! be composed with the transport codecs in `rtd-proto`.
//!
//! [`secret`] parses the human MTProxy secret string into its obfuscation mode
//! (plain, secured `dd`, or FakeTLS `ee`) and key, so a configuration string can
//! be composed with the layer it selects.

pub mod error;
pub mod faketls;
pub mod obfuscated2;
pub mod secret;

pub use error::{Error, Result};
pub use faketls::{
    FakeTls, MAX_HANDSHAKE_RECORDS, RANDOM_LENGTH, RANDOM_OFFSET, RECORD_TYPE_ALERT,
    RECORD_TYPE_APPLICATION, RECORD_TYPE_CHANGE_CIPHER_SPEC, RECORD_TYPE_HANDSHAKE,
    RECORD_TYPE_HEARTBEAT, Record, VERSION_10, VERSION_11, VERSION_12, VERSION_13,
    client_handshake, generate_client_hello, read_record, read_server_hello, verify_client_hello,
    write_client_hello, write_record, write_server_hello,
};
pub use obfuscated2::{Metadata, Obfuscated2};
pub use secret::{Secret, SecretMode};
