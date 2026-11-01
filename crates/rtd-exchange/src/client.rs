//! The client side of the authorization-key exchange.

use num_bigint::BigUint;
use rsa::RsaPublicKey;
use rtd_bin::Writer;
use rtd_crypto::{AuthKey, RandomSource};
use rtd_mt::{
    Client_DH_Inner_Data, P_Q_inner_data, Req_DH_params, Req_pq_multi, ResPQ, Server_DH_Params,
    Server_DH_inner_data, Set_client_DH_params, Set_client_DH_params_answer,
};
use rtd_proto::MessageType;
use rtd_util::Clock;

use crate::Stream;
use crate::error::{Error, Result};
use crate::stream::{MessageIds, read_unencrypted, write_unencrypted};
use crate::util::{
    biguint_to_key, decode_with, random_biguint, random_i64, random_int128, random_int256,
};

/// Client-side exchange configuration.
pub struct ClientConfig {
    /// Target datacenter id.
    pub dc: i32,
    /// Whether to request a temporary (PFS) key instead of a permanent one.
    pub temporary: bool,
    /// Temporary-key lifetime in seconds, used when `temporary` is set.
    pub expires_in: i32,
    /// Candidate server public keys; the one matching the server fingerprint is
    /// used.
    pub public_keys: Vec<RsaPublicKey>,
}

/// Result of a completed client exchange.
#[derive(Debug)]
pub struct ClientExchangeResult {
    /// The established authorization key.
    pub auth_key: AuthKey,
    /// A freshly generated session id.
    pub session_id: i64,
    /// The initial server salt.
    pub server_salt: i64,
}

/// Runs the client side of the authorization-key exchange.
///
/// # Errors
///
/// Returns an error when the stream fails, the server's responses fail
/// validation, or a cryptographic step fails.
pub fn client_exchange(
    stream: &mut impl Stream,
    config: &ClientConfig,
    clock: &dyn Clock,
    random: &mut impl RandomSource,
) -> Result<ClientExchangeResult> {
    let mut ids = MessageIds::new();

    // 1. req_pq_multi.
    let nonce = random_int128(random);
    let mut writer = Writer::new();
    Req_pq_multi { nonce }.encode(&mut writer);
    write_unencrypted(
        stream,
        &mut ids,
        clock,
        MessageType::FromClient,
        writer.as_slice(),
    )?;

    // 2. resPQ.
    let data = read_unencrypted(stream, MessageType::ServerResponse)?;
    let res = decode_with(&data, ResPQ::decode)?;
    if res.nonce != nonce {
        return Err(Error::NonceMismatch);
    }
    let server_nonce = res.server_nonce;

    let selected = config
        .public_keys
        .iter()
        .find(|key| {
            res.server_public_key_fingerprints
                .contains(&rtd_crypto::rsa_fingerprint(key))
        })
        .ok_or(Error::KeyFingerprintNotFound)?;

    // 3. Factor pq (at most 2^63).
    let pq = BigUint::from_bytes_be(&res.pq);
    if pq.bits() > 63 {
        return Err(Error::BadPq);
    }
    let (p, q) = rtd_crypto::decompose_pq(&pq, random);
    let p_bytes = p.to_bytes_be();
    let q_bytes = q.to_bytes_be();

    // 4. req_DH_params with RSA-padded inner data.
    let new_nonce = random_int256(random);
    let inner = if config.temporary {
        P_Q_inner_data::P_q_inner_data_temp_dc {
            pq: res.pq.clone(),
            p: p_bytes.clone(),
            q: q_bytes.clone(),
            nonce,
            server_nonce,
            new_nonce,
            dc: config.dc,
            expires_in: config.expires_in,
        }
    } else {
        P_Q_inner_data::P_q_inner_data_dc {
            pq: res.pq.clone(),
            p: p_bytes.clone(),
            q: q_bytes.clone(),
            nonce,
            server_nonce,
            new_nonce,
            dc: config.dc,
        }
    };
    let mut writer = Writer::new();
    inner.encode(&mut writer);
    let encrypted_data = rtd_crypto::rsa_pad(writer.as_slice(), selected, random)?;

    let request = Req_DH_params {
        nonce,
        server_nonce,
        p: p_bytes,
        q: q_bytes,
        public_key_fingerprint: rtd_crypto::rsa_fingerprint(selected),
        encrypted_data: encrypted_data.to_vec(),
    };
    let mut writer = Writer::new();
    request.encode(&mut writer);
    write_unencrypted(
        stream,
        &mut ids,
        clock,
        MessageType::FromClient,
        writer.as_slice(),
    )?;

    // 5. server_DH_params.
    let data = read_unencrypted(stream, MessageType::ServerResponse)?;
    let encrypted_answer = match decode_with(&data, Server_DH_Params::decode)? {
        Server_DH_Params::Server_DH_params_ok {
            nonce: got_nonce,
            server_nonce: got_server_nonce,
            encrypted_answer,
        } => {
            if got_nonce != nonce || got_server_nonce != server_nonce {
                return Err(Error::NonceMismatch);
            }
            encrypted_answer
        }
        Server_DH_Params::Server_DH_params_fail { .. } => return Err(Error::ServerRefused),
    };

    let new_nonce_bytes = new_nonce.as_array();
    let server_nonce_bytes = server_nonce.as_array();
    let (key, iv) = rtd_crypto::temp_aes_keys(new_nonce_bytes, server_nonce_bytes);
    let answer = rtd_crypto::decrypt_exchange_answer(&encrypted_answer, &key, &iv)?;
    let inner_data = decode_with(&answer, Server_DH_inner_data::decode)?;
    if inner_data.nonce != nonce || inner_data.server_nonce != server_nonce {
        return Err(Error::NonceMismatch);
    }

    let dh_prime = BigUint::from_bytes_be(&inner_data.dh_prime);
    let g = inner_data.g;
    rtd_crypto::check_dh(u32::try_from(g).map_err(|_| Error::BadPq)?, &dh_prime)?;
    let g_a = BigUint::from_bytes_be(&inner_data.g_a);

    // 6. Choose b and send set_client_DH_params.
    let b = random_biguint(random);
    let g_big = BigUint::from(u64::try_from(g).map_err(|_| Error::BadPq)?);
    let g_b = g_big.modpow(&b, &dh_prime);
    rtd_crypto::check_dh_params(&dh_prime, &g_big, &g_a, &g_b)?;

    let client_inner = Client_DH_Inner_Data {
        server_nonce: inner_data.server_nonce,
        nonce: inner_data.nonce,
        retry_id: 0,
        g_b: g_b.to_bytes_be(),
    };
    let mut writer = Writer::new();
    client_inner.encode(&mut writer);
    let client_encrypted =
        rtd_crypto::encrypt_exchange_answer(random, writer.as_slice(), &key, &iv);

    let set_params = Set_client_DH_params {
        nonce,
        server_nonce: request.server_nonce,
        encrypted_data: client_encrypted,
    };
    let mut writer = Writer::new();
    set_params.encode(&mut writer);
    write_unencrypted(
        stream,
        &mut ids,
        clock,
        MessageType::FromClient,
        writer.as_slice(),
    )?;

    // 7. Compute auth_key = g_a^b mod p and verify dh_gen_ok.
    let auth_key_value = g_a.modpow(&b, &dh_prime);
    let key = biguint_to_key(&auth_key_value)?;

    let data = read_unencrypted(stream, MessageType::ServerResponse)?;
    match decode_with(&data, Set_client_DH_params_answer::decode)? {
        Set_client_DH_params_answer::Dh_gen_ok {
            nonce: got_nonce,
            server_nonce: got_server_nonce,
            new_nonce_hash1,
        } => {
            if got_nonce != nonce || got_server_nonce != server_nonce {
                return Err(Error::NonceMismatch);
            }
            if rtd_crypto::nonce_hash1(new_nonce_bytes, &key) != new_nonce_hash1 {
                return Err(Error::HashMismatch);
            }
            Ok(ClientExchangeResult {
                auth_key: AuthKey::new(key),
                session_id: random_i64(random),
                server_salt: rtd_crypto::server_salt(new_nonce_bytes, server_nonce_bytes),
            })
        }
        Set_client_DH_params_answer::Dh_gen_retry { .. }
        | Set_client_DH_params_answer::Dh_gen_fail { .. } => Err(Error::ServerRefused),
    }
}
