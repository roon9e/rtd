//! Two-factor (SRP) authorization against a mock server that verifies `M1`.
//!
//! The server builds a real SRP verifier and challenge from a password and
//! checks the client's proof with the same primitives the reference server
//! would use.

use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_bin::Writer;
use rtd_client::{Client, DuplexStream, Error, Transport};
use rtd_crypto::srp::{self, Input};
use rtd_crypto::{EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_tg::{
    AccountPassword, AuthAuthorization, Bool, DecodeLimits, Decoder, InputCheckPasswordSRP, Method,
    Object, PasswordKdfAlgo, SecurePasswordKdfAlgo,
};
use rtd_util::{Clock, SystemClock};

/// The 2048-bit SRP prime used by the reference tests.
const SRP_PRIME_HEX: &str = concat!(
    "C71CAEB9C6B1C9048E6C522F70F13F73980D40238E3E21C14934D037563D930F",
    "48198A0AA7C14058229493D22530F4DBFA336F6E0AC925139543AED44CCE7C37",
    "20FD51F69458705AC68CD4FE6B6B13ABDC9746512969328454F18FAF8C595F64",
    "2477FE96BB2A941D5BCD1D4AC8CC49880708FA9B378E3C4F3A9060BEE67CF9A4",
    "A4A695811051907E162753B56B0F6B410DBA74D8A84B2A14B3144E0EF1284754",
    "FD17ED950D5965B4B9DD46582DB1178D169C6BC465B0D6FF9CA3928FEF5B9AE4",
    "E418FC15E83EBEA0F87FA9FF5EED70050DED2849F47BF959D956850CE929851F",
    "0D8115F635B105EE2E4E15D04B2454BF6F4FADF034B10403119CD8E3B92FCC5B",
);

struct ServerStream(DuplexStream);

impl ExchangeStream for ServerStream {
    fn send(&mut self, frame: &[u8]) -> rtd_exchange::Result<()> {
        self.0
            .send_frame(frame)
            .map_err(|error| rtd_exchange::Error::Stream(error.to_string()))
    }

    fn recv(&mut self) -> rtd_exchange::Result<Vec<u8>> {
        self.0
            .recv_frame()
            .map_err(|error| rtd_exchange::Error::Stream(error.to_string()))
    }
}

fn hex_vec(hex: &str) -> Vec<u8> {
    hex::decode(hex).expect("valid hex")
}

fn srp_input() -> Input {
    Input {
        salt1: hex_vec(
            "4D11FB6BEC38F9D2546BB0F61E4F1C99A1BC0DB8F0D5F35B1291B37B213123D7ED48F3C6794D495B",
        ),
        salt2: hex_vec("A1B181AAFE88188680AE32860D60BB01"),
        g: 3,
        p: hex_vec(SRP_PRIME_HEX),
    }
}

fn encode(object: &Object) -> Vec<u8> {
    let mut writer = Writer::new();
    object.encode(&mut writer);
    writer.into_vec()
}

fn unwrap_method(method: &Method) -> &Method {
    match method {
        Method::InvokeWithLayer(wrapper) => unwrap_method(&wrapper.query),
        Method::InitConnection(wrapper) => unwrap_method(&wrapper.query),
        other => other,
    }
}

fn account_password(input: &Input, srp_b: Vec<u8>) -> Vec<u8> {
    encode(&Object::AccountPassword(Box::new(AccountPassword {
        flags: 0,
        has_recovery: false,
        has_secure_values: false,
        has_password: true,
        current_algo: Some(Box::new(
            PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow {
                salt1: input.salt1.clone(),
                salt2: input.salt2.clone(),
                g: input.g,
                p: input.p.clone(),
            },
        )),
        srp_b: Some(srp_b),
        srp_id: Some(7),
        hint: None,
        email_unconfirmed_pattern: None,
        new_algo: Box::new(PasswordKdfAlgo::PasswordKdfAlgoUnknown),
        new_secure_algo: Box::new(SecurePasswordKdfAlgo::SecurePasswordKdfAlgoUnknown),
        secure_random: Vec::new(),
        pending_reset_date: None,
        login_email_pattern: None,
    })))
}

/// Runs the exchange, then answers `account.getPassword` and `auth.checkPassword`,
/// verifying the client's proof for `password`.
fn spawn_server(
    private: RsaPrivateKey,
    server_stream: DuplexStream,
    password: Vec<u8>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let clock = SystemClock;
        let mut stream = ServerStream(server_stream);
        let mut parameters = TestServerRng;
        let mut random = SystemRandom;
        let result = server_exchange(
            &mut stream,
            &ServerConfig {
                dc: 2,
                key: &private,
                strict_dc: true,
            },
            &mut parameters,
            &clock,
            &mut random,
        )
        .expect("server exchange");
        let auth_key = result.auth_key;
        let server_salt = result.server_salt;

        let input = srp_input();
        let v = srp::verifier(&password, &input.salt1, &input.salt2, &input).expect("verifier");
        let challenge = srp::challenge(&v, &input, &mut SystemRandom).expect("challenge");
        let verifier = v.clone();
        let secret = challenge.secret.clone();
        let srp_b = challenge.public.clone();

        let mut server_msg_id = 0i64;
        while let Ok(frame) = stream.0.recv_frame() {
            let message = EncryptedMessage::decode(&frame).expect("decode client frame");
            let decrypted = decrypt(&auth_key, &message, Side::Client).expect("decrypt client");
            let object = {
                let mut decoder = Decoder::new(&decrypted.body, DecodeLimits::default());
                Method::decode(&mut decoder)
            };
            let payload = match object {
                Ok(method) => match unwrap_method(&method) {
                    Method::AccountGetPassword(_) => account_password(&input, srp_b.clone()),
                    Method::AuthCheckPassword(check) => match &*check.password {
                        InputCheckPasswordSRP::InputCheckPasswordSRP { a, m1, .. } => {
                            let ok = srp::verify(&verifier, &secret, a, m1, &input)
                                .unwrap_or(None)
                                .is_some();
                            if ok {
                                encode(&Object::AuthAuthorization(Box::new(
                                    AuthAuthorization::AuthorizationSignUpRequired {
                                        flags: 0,
                                        terms_of_service: None,
                                    },
                                )))
                            } else {
                                encode(&Object::Bool(Box::new(Bool::BoolTrue)))
                            }
                        }
                        InputCheckPasswordSRP::InputCheckPasswordEmpty => {
                            encode(&Object::Bool(Box::new(Bool::BoolTrue)))
                        }
                    },
                    _ => encode(&Object::Bool(Box::new(Bool::BoolTrue))),
                },
                Err(_) => encode(&Object::Bool(Box::new(Bool::BoolTrue))),
            };
            let result = rtd_proto::RpcResult::new(decrypted.header.msg_id, payload).encode();
            server_msg_id += 1;
            let header = MessageHeader {
                salt: server_salt,
                session_id: decrypted.header.session_id,
                msg_id: server_msg_id,
                seq_no: decrypted.header.seq_no,
            };
            let reply = encrypt(&auth_key, &header, &result, Side::Server, &mut random).encode();
            if stream.0.send_frame(&reply).is_err() {
                break;
            }
        }
    })
}

fn keypair(seed: u64) -> (RsaPrivateKey, rsa::RsaPublicKey) {
    let mut rng = StdRng::seed_from_u64(seed);
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
    let public = private.to_public_key();
    (private, public)
}

fn config(public: rsa::RsaPublicKey) -> ClientConfig {
    ClientConfig {
        dc: 2,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public],
    }
}

fn clock() -> Arc<dyn Clock + Send + Sync> {
    Arc::new(SystemClock)
}

#[test]
fn check_password_completes_on_a_valid_proof() {
    let (private, public) = keypair(61);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream, b"correct horse".to_vec());

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");
    let authorization = client
        .check_password(b"correct horse")
        .expect("valid password");
    assert!(matches!(
        authorization,
        AuthAuthorization::AuthorizationSignUpRequired { .. }
    ));

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn check_password_wrong_password_is_rejected() {
    let (private, public) = keypair(67);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream, b"correct horse".to_vec());

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");
    let error = client
        .check_password(b"wrong password")
        .expect_err("wrong password must fail");
    assert!(matches!(error, Error::Unexpected(_)), "{error:?}");

    drop(client);
    server.join().expect("server thread");
}
