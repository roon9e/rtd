//! The server side of the authorization-key exchange.

use core::time::Duration;
use std::time::UNIX_EPOCH;

use num_bigint::BigUint;
use rsa::RsaPrivateKey;
use rtd_bin::Writer;
use rtd_crypto::{AuthKey, RandomSource};
use rtd_mt::{
    Client_DH_Inner_Data, P_Q_inner_data, Req_DH_params, Req_pq, Req_pq_multi, ResPQ,
    Server_DH_Params, Server_DH_inner_data, Set_client_DH_params, Set_client_DH_params_answer,
};
use rtd_proto::MessageType;
use rtd_util::Clock;

use crate::Stream;
use crate::error::{Error, Result};
use crate::rng::ServerRng;
use crate::stream::{MessageIds, decode_unencrypted, frame_auth_key_id, write_unencrypted};
use crate::util::{biguint_to_key, decode_with, peek_id, random_int128};

/// Server-side exchange configuration.
pub struct ServerConfig<'a> {
    /// The datacenter this server serves.
    pub dc: i32,
    /// The server's RSA private key.
    pub key: &'a RsaPrivateKey,
    /// Whether to reject a client that selects a different datacenter label.
    ///
    /// The reference edge runs with this **off** (`strict_dc_check: false`): a
    /// single physical backend accepts every wire datacenter label, and the
    /// label never changes key persistence or session identity. Clients commonly
    /// try several labels (for example 2, 3, 4) before one succeeds, so a strict
    /// server drops their handshakes.
    pub strict_dc: bool,
}

/// Result of a completed server exchange.
#[derive(Debug)]
pub struct ServerExchangeResult {
    /// The established authorization key.
    pub auth_key: AuthKey,
    /// The initial server salt.
    pub server_salt: i64,
    /// The key's absolute expiry as a Unix timestamp, or `0` when the client
    /// requested a persistent key.
    ///
    /// A client that asks for a temporary key (`p_q_inner_data_temp_dc`) must
    /// have it persisted with a live expiry: the core only accepts
    /// `auth.bindTempAuthKey` when the temporary key is not yet expired and the
    /// permanent key never expires.
    pub expires_at: i32,
}

/// Runs the server side of the authorization-key exchange.
///
/// # Errors
///
/// Returns an error when the stream fails, the client's messages fail
/// validation, the client requests a different datacenter, or a cryptographic
/// step fails. [`Error::UnexpectedEncrypted`] means the peer is using an
/// already-established key and the frame must be handled as an encrypted message
/// rather than answering that the key is unknown.
pub fn server_exchange(
    stream: &mut impl Stream,
    config: &ServerConfig<'_>,
    rng: &mut impl ServerRng,
    clock: &dyn Clock,
    random: &mut impl RandomSource,
) -> Result<ServerExchangeResult> {
    let mut ids = MessageIds::new();
    let mut client_nonce = None;
    let mut server_nonce = None;

    // 1. req_pq / req_pq_multi, then req_DH_params. A client may repeat req_pq.
    let request = loop {
        let frame = stream.recv()?;
        let auth_key_id = frame_auth_key_id(&frame)?;
        if auth_key_id != 0 {
            return Err(Error::UnexpectedEncrypted { auth_key_id });
        }
        let data = decode_unencrypted(&frame, MessageType::FromClient)?;
        match peek_id(&data)? {
            Req_pq_multi::TYPE_ID | Req_pq::TYPE_ID => {
                let nonce = if peek_id(&data)? == Req_pq_multi::TYPE_ID {
                    decode_with(&data, Req_pq_multi::decode)?.nonce
                } else {
                    decode_with(&data, Req_pq::decode)?.nonce
                };
                let fresh_server_nonce = random_int128(random);
                let pq = rng.pq(random)?;
                let response = ResPQ {
                    nonce,
                    server_nonce: fresh_server_nonce,
                    pq: pq.to_bytes_be(),
                    server_public_key_fingerprints: vec![rtd_crypto::rsa_fingerprint(
                        &config.key.to_public_key(),
                    )],
                };
                let mut writer = Writer::new();
                response.encode(&mut writer);
                write_unencrypted(
                    stream,
                    &mut ids,
                    clock,
                    MessageType::ServerResponse,
                    writer.as_slice(),
                )?;
                client_nonce = Some(nonce);
                server_nonce = Some(fresh_server_nonce);
            }
            Req_DH_params::TYPE_ID => break decode_with(&data, Req_DH_params::decode)?,
            other => {
                return Err(Error::Decode(format!(
                    "unexpected constructor {other:#010x} during key exchange"
                )));
            }
        }
    };

    let client_nonce = client_nonce.ok_or(Error::NonceMismatch)?;
    let server_nonce = server_nonce.ok_or(Error::NonceMismatch)?;

    // 2. Unwrap the RSA-padded inner data and validate the requested datacenter.
    let inner_bytes = rtd_crypto::decode_rsa_pad(&request.encrypted_data, config.key)?;
    let inner = decode_with(&inner_bytes, P_Q_inner_data::decode)?;
    let (nonce, request_server_nonce, new_nonce, expires_in) = match inner {
        P_Q_inner_data::P_q_inner_data {
            nonce,
            server_nonce,
            new_nonce,
            ..
        } => (nonce, server_nonce, new_nonce, None),
        P_Q_inner_data::P_q_inner_data_dc {
            nonce,
            server_nonce,
            new_nonce,
            dc,
            ..
        } => (
            nonce,
            server_nonce,
            accept_dc(config, dc).map(|()| new_nonce)?,
            None,
        ),
        P_Q_inner_data::P_q_inner_data_temp_dc {
            nonce,
            server_nonce,
            new_nonce,
            dc,
            expires_in,
            ..
        } => (
            nonce,
            server_nonce,
            accept_dc(config, dc).map(|()| new_nonce)?,
            Some(expires_in),
        ),
    };
    if nonce != client_nonce || request_server_nonce != server_nonce {
        return Err(Error::NonceMismatch);
    }

    // 3. Generate DH parameters and send server_DH_params_ok.
    let dh_prime = rng.dh_prime()?;
    const G: u32 = 3;
    let (a, g_a) = rng.ga(G, &dh_prime, random)?;
    let response = Server_DH_inner_data {
        nonce: client_nonce,
        server_nonce,
        g: i32::try_from(G).expect("g fits an i32"),
        dh_prime: dh_prime.to_bytes_be(),
        g_a: g_a.to_bytes_be(),
        server_time: unix_seconds(clock),
    };
    let mut writer = Writer::new();
    response.encode(&mut writer);

    let new_nonce_bytes = new_nonce.as_array();
    let server_nonce_bytes = server_nonce.as_array();
    let (key, iv) = rtd_crypto::temp_aes_keys(new_nonce_bytes, server_nonce_bytes);
    let encrypted_answer =
        rtd_crypto::encrypt_exchange_answer(random, writer.as_slice(), &key, &iv);
    let ok = Server_DH_Params::Server_DH_params_ok {
        nonce: client_nonce,
        server_nonce,
        encrypted_answer,
    };
    let mut writer = Writer::new();
    ok.encode(&mut writer);
    write_unencrypted(
        stream,
        &mut ids,
        clock,
        MessageType::ServerResponse,
        writer.as_slice(),
    )?;

    // 4. Read set_client_DH_params and derive the shared key.
    let frame = stream.recv()?;
    let data = decode_unencrypted(&frame, MessageType::FromClient)?;
    let set_params = decode_with(&data, Set_client_DH_params::decode)?;
    let decrypted = rtd_crypto::decrypt_exchange_answer(&set_params.encrypted_data, &key, &iv)?;
    let client_inner = decode_with(&decrypted, Client_DH_Inner_Data::decode)?;

    let g_b = BigUint::from_bytes_be(&client_inner.g_b);
    let auth_key_value = g_b.modpow(&a, &dh_prime);
    let key_value = biguint_to_key(&auth_key_value)?;

    // 5. Send dh_gen_ok.
    let answer = Set_client_DH_params_answer::Dh_gen_ok {
        nonce: client_nonce,
        server_nonce,
        new_nonce_hash1: rtd_crypto::nonce_hash1(new_nonce_bytes, &key_value),
    };
    let mut writer = Writer::new();
    answer.encode(&mut writer);
    write_unencrypted(
        stream,
        &mut ids,
        clock,
        MessageType::ServerResponse,
        writer.as_slice(),
    )?;

    let expires_at = expires_in.map_or(0, |secs| unix_seconds(clock) + secs);
    Ok(ServerExchangeResult {
        auth_key: AuthKey::new(key_value),
        server_salt: rtd_crypto::server_salt(new_nonce_bytes, server_nonce_bytes),
        expires_at,
    })
}

/// Validates the requested datacenter.
fn check_dc(want: i32, got: i32) -> Result<()> {
    if want == got {
        Ok(())
    } else {
        Err(Error::WrongDc { want, got })
    }
}

/// Accepts a client-selected datacenter label, honouring `strict_dc`.
fn accept_dc(config: &ServerConfig<'_>, got: i32) -> Result<()> {
    if config.strict_dc {
        check_dc(config.dc, got)
    } else {
        Ok(())
    }
}

/// Returns the current Unix time in seconds, saturating at the `i32` bounds.
fn unix_seconds(clock: &dyn Clock) -> i32 {
    let seconds = clock
        .now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    i32::try_from(seconds).unwrap_or(i32::MAX)
}
