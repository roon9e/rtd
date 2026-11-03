//! MTProto cryptography primitives.
#![warn(missing_docs)]
//!
//! This crate implements the server-facing cryptographic core of the MTProto
//! protocol, mirroring the behaviour of the reference Go implementation in
//! `td/crypto` byte for byte so that the two are wire-compatible:
//!
//! - [`ige`] — AES-256 in infinite-garble-extension (IGE) mode.
//! - [`encrypted_message`] — the MTProto 2.0 encrypted-message envelope,
//!   including its inner header and random padding.
//! - [`kdf`] — the MTProto 2.0 key-derivation function (message key, AES key
//!   and IGE initialization vector) with a side-dependent offset.
//! - [`kdf_v1`] — the legacy MTProto 1.0 key-derivation function still required
//!   for the temporary-auth-key binding envelope.
//! - [`key`] — the 2048-bit authorization key and its 64-bit identifier.
//! - [`bind`] — the `bind_auth_key_inner` payload and its v1 envelope.
//! - [`rsa`] — RSA exchange padding, the inverse operation, PEM key parsing and
//!   the MTProto RSA fingerprint.
//! - [`dh`] — Diffie-Hellman parameter validation (range, generator and safe
//!   prime checks).
//! - [`pq`] — factorization of the client's composite proof-of-work number.
//! - [`srp`] — the SRP-based two-factor authentication hashing and proof.
//! - [`temp_keys`] — temporary exchange keys, nonce hash and server salt.
//! - [`rand`] — cryptographically secure randomness.
//!
//! All randomness used for keys, nonces and padding is drawn from the operating
//! system CSPRNG (see [`rand`]).

pub mod bind;
pub mod cbc;
pub mod ctr;
pub mod dh;
pub mod encrypted_message;
pub mod error;
pub mod ige;
pub mod kdf;
pub mod kdf_v1;
pub mod key;
pub mod pq;
pub mod rand;
pub mod rsa;
pub mod srp;
pub mod temp_keys;

#[cfg(test)]
mod testutil;

pub use bind::{BIND_AUTH_KEY_INNER_TYPE_ID, BindAuthKeyInner, encrypt_bind_message};
pub use cbc::{decrypt as cbc_decrypt, encrypt as cbc_encrypt};
pub use ctr::xor as ctr_xor;
pub use dh::{DH_PRIME_BITS, check_dh, check_dh_params, check_gp, check_prime, in_range};
pub use encrypted_message::{
    DecryptedMessage, EncryptedMessage, MessageHeader, auth_key_id, decrypt, encrypt,
};
pub use error::{Error, Result};
pub use ige::{decrypt as ige_decrypt, encrypt as ige_encrypt};
pub use kdf::{Side, keys, message_key};
pub use kdf_v1::{keys_v1, message_key_v1};
pub use key::{AuthKey, Key};
pub use pq::decompose_pq;
pub use rand::{RandomSource, SystemRandom, fill_random, random_bytes};
pub use rsa::{
    decode_rsa_pad, parse_rsa_private_key, parse_rsa_public_key, parse_rsa_public_keys,
    rsa_fingerprint, rsa_pad, rsa_public_transform,
};
pub use srp::{
    Answer, Challenge, Input, challenge, hash as srp_hash, new_hash, new_hash_with_random,
    verifier, verify,
};
pub use temp_keys::{
    decrypt_exchange_answer, encrypt_exchange_answer, nonce_hash1, server_salt, temp_aes_keys,
};
