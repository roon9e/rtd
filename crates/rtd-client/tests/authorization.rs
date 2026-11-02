//! The authorization flow (`auth.sendCode` -> `auth.signIn`) against a mock
//! server that dispatches on the unwrapped method id.

use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_bin::Writer;
use rtd_client::{AsyncClient, Client, DuplexStream, Error, InitParams, Transport};
use rtd_crypto::{EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_tg::{
    AuthAuthorization, AuthSendCode, AuthSentCode, AuthSentCodeType, AuthSignIn, Bool,
    CodeSettings, DecodeLimits, Decoder, Method, Object,
};
use rtd_util::{Clock, SystemClock};

/// A mock server's answer for one request body.
type Reply = fn(&[u8]) -> Option<Vec<u8>>;

/// Adapts the client's duplex to the exchange `Stream` the server flow expects.
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

/// Unwraps `invokeWithLayer`/`initConnection` to the caller's method.
fn unwrap_method(method: &Method) -> &Method {
    match method {
        Method::InvokeWithLayer(wrapper) => unwrap_method(&wrapper.query),
        Method::InitConnection(wrapper) => unwrap_method(&wrapper.query),
        other => other,
    }
}

/// Encodes one canonical object as a result body.
fn encode(object: &Object) -> Vec<u8> {
    let mut writer = Writer::new();
    object.encode(&mut writer);
    writer.into_vec()
}

/// Answers each method the client asks for with its expected result type.
fn reply_dispatch(body: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = Decoder::new(body, DecodeLimits::default());
    let method = Method::decode(&mut decoder).ok()?;
    Some(match unwrap_method(&method).type_id() {
        AuthSendCode::TYPE_ID => encode(&Object::AuthSentCode(Box::new(AuthSentCode::SentCode {
            flags: 0,
            type_: Box::new(AuthSentCodeType::SentCodeTypeApp { length: 5 }),
            phone_code_hash: "login-hash".to_owned(),
            next_type: None,
            timeout: None,
        }))),
        AuthSignIn::TYPE_ID => encode(&Object::AuthAuthorization(Box::new(
            AuthAuthorization::AuthorizationSignUpRequired {
                flags: 0,
                terms_of_service: None,
            },
        ))),
        _ => encode(&Object::Bool(Box::new(Bool::BoolTrue))),
    })
}

/// Answers every method with a `Bool`, so a typed authorization call fails.
fn reply_bool(body: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = Decoder::new(body, DecodeLimits::default());
    Method::decode(&mut decoder).ok()?;
    Some(encode(&Object::Bool(Box::new(Bool::BoolTrue))))
}

/// Runs the server exchange, then answers every client request with `reply`.
fn spawn_server(
    private: RsaPrivateKey,
    server_stream: DuplexStream,
    reply: Reply,
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

        let mut server_msg_id = 0i64;
        while let Ok(frame) = stream.0.recv_frame() {
            let message = EncryptedMessage::decode(&frame).expect("decode client frame");
            let decrypted = decrypt(&auth_key, &message, Side::Client).expect("decrypt client");
            let Some(result_body) = reply(&decrypted.body) else {
                continue;
            };
            let result = rtd_proto::RpcResult::new(decrypted.header.msg_id, result_body).encode();
            server_msg_id += 1;
            let header = MessageHeader {
                salt: server_salt,
                session_id: decrypted.header.session_id,
                msg_id: server_msg_id,
                seq_no: decrypted.header.seq_no,
            };
            let reply = encrypt(&auth_key, &header, &result, Side::Server, &mut random).encode();
            stream.0.send_frame(&reply).expect("send reply");
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

fn params() -> InitParams {
    InitParams {
        api_id: 2040,
        device_model: "rtd".to_owned(),
        system_version: "test".to_owned(),
        app_version: "0.0.0".to_owned(),
        system_lang_code: "en".to_owned(),
        lang_pack: String::new(),
        lang_code: "en".to_owned(),
    }
}

fn settings() -> CodeSettings {
    CodeSettings {
        flags: 0,
        allow_flashcall: false,
        current_number: false,
        allow_app_hash: false,
        allow_missed_call: false,
        allow_firebase: false,
        unknown_number: false,
        logout_tokens: None,
        token: None,
        app_sandbox: None,
    }
}

fn start(seed: u64, reply: Reply) -> (Client<DuplexStream>, std::thread::JoinHandle<()>) {
    let (private, public) = keypair(seed);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream, reply);
    let client = Client::connect(client_stream, &config(public), clock()).expect("connect");
    (client, server)
}

fn clock() -> Arc<dyn Clock + Send + Sync> {
    Arc::new(SystemClock)
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(future)
}

#[test]
fn login_flow_returns_typed_results() {
    let (mut client, server) = start(29, reply_dispatch);
    client.initialize(params()).expect("initialize");
    let sent = client
        .send_code("+10000000000", 2040, "api-hash", settings())
        .expect("sendCode");
    let AuthSentCode::SentCode {
        phone_code_hash, ..
    } = sent
    else {
        panic!("expected auth.sentCode");
    };
    assert_eq!(phone_code_hash, "login-hash");
    let authorization = client
        .sign_in("+10000000000", &phone_code_hash, Some("12345"))
        .expect("signIn");
    assert!(matches!(
        authorization,
        AuthAuthorization::AuthorizationSignUpRequired { .. }
    ));

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn an_unexpected_result_fails_closed() {
    // The mock answers every method with a `Bool`, so a typed authorization call
    // must reject the mismatched constructor.
    let (mut client, server) = start(37, reply_bool);
    client.initialize(params()).expect("initialize");
    let error = client
        .send_code("+1", 1, "x", settings())
        .expect_err("a Bool result is not an auth.SentCode");
    assert!(matches!(error, Error::Unexpected(_)), "{error:?}");

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn authorization_is_reachable_through_the_async_facade() {
    let (private, public) = keypair(31);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream, reply_dispatch);

    let client = AsyncClient::spawn(client_stream, config(public), clock()).expect("spawn");
    let sent = block_on(client.invoke(Method::AuthSendCode(Box::new(AuthSendCode {
        phone_number: "+10000000000".to_owned(),
        api_id: 2040,
        api_hash: "api-hash".to_owned(),
        settings: Box::new(settings()),
    }))))
    .expect("async sendCode");
    let Object::AuthSentCode(code) = sent else {
        panic!("expected auth.sentCode");
    };
    assert!(matches!(*code, AuthSentCode::SentCode { .. }));

    drop(client);
    server.join().expect("server thread");
}
