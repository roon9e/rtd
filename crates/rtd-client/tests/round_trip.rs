//! Client round trip against an in-process server: the auth-key exchange, an
//! encrypted `invokeWithLayer`/`initConnection` request, and `rpc_result`
//! correlation, all over an in-memory duplex — synchronously and through the
//! asynchronous façade.

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
use rtd_tg::{Bool, Object};
use rtd_util::{Clock, SystemClock};

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

/// Runs the server exchange over `server_stream`, then answers the client's
/// first content request with a known `Bool` result.
fn spawn_server(
    private: RsaPrivateKey,
    server_stream: DuplexStream,
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
            },
            &mut parameters,
            &clock,
            &mut random,
        )
        .expect("server exchange");
        let auth_key = result.auth_key;
        let server_salt = result.server_salt;

        let mut replied = false;
        let mut server_msg_id = 0i64;
        while let Ok(frame) = stream.0.recv_frame() {
            let message = EncryptedMessage::decode(&frame).expect("decode client frame");
            let decrypted = decrypt(&auth_key, &message, Side::Client).expect("decrypt client");
            if replied {
                continue;
            }
            let mut writer = Writer::new();
            Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut writer);
            let result =
                rtd_proto::RpcResult::new(decrypted.header.msg_id, writer.into_vec()).encode();
            server_msg_id += 1;
            let header = MessageHeader {
                salt: server_salt,
                session_id: decrypted.header.session_id,
                msg_id: server_msg_id,
                seq_no: decrypted.header.seq_no,
            };
            let reply = encrypt(&auth_key, &header, &result, Side::Server, &mut random).encode();
            stream.0.send_frame(&reply).expect("send reply");
            replied = true;
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

fn clock() -> Arc<dyn Clock + Send + Sync> {
    Arc::new(SystemClock)
}

/// Runs a future on a current-thread runtime (avoids the `macros`/proc-macro
/// dependency).
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(future)
}

#[test]
fn client_completes_exchange_and_invokes() {
    let (private, public) = keypair(19);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client = Client::connect(client_stream, &config(public), clock())
        .expect("client exchange completes");
    let result = client
        .initialize(params())
        .expect("initialize returns a result");
    assert!(
        matches!(result, Object::Bool(_)),
        "expected the server's Bool result, got {result:?}"
    );

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn async_client_completes_exchange_and_invokes() {
    let (private, public) = keypair(23);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let client = AsyncClient::spawn(client_stream, config(public), clock()).expect("spawn");
    let result = block_on(client.initialize(params())).expect("async initialize");
    assert!(
        matches!(result, Object::Bool(_)),
        "expected the server's Bool result, got {result:?}"
    );

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn async_client_reports_a_startup_failure() {
    let (private, public) = keypair(17);
    let (client_stream, server_stream) = DuplexStream::pair();
    // Drop the server end without exchanging, so `spawn` cannot complete.
    drop(server_stream);
    drop(private);

    let error = AsyncClient::spawn(client_stream, config(public), clock())
        .err()
        .expect("spawn fails when the peer is gone");
    assert!(matches!(error, Error::Exchange(_)), "{error:?}");
}
