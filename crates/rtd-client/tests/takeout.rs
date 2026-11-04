//! Takeout session integration tests against an in-process server.
//!
//! The server answers `account.initTakeoutSession`, the wrapped calls and
//! `account.finishTakeoutSession`, recording that the session was finished.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_bin::Writer;
use rtd_client::takeout::{self, TakeoutConfig};
use rtd_client::{Client, DuplexStream, Error, Transport};
use rtd_crypto::{EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_tg::{AccountTakeout, Bool, HelpGetConfig, Method, Object};
use rtd_util::{Clock, SystemClock};

/// Adapts the duplex to the exchange stream interface.
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

/// Runs the exchange, then answers takeout methods until the peer hangs up.
fn spawn_takeout_server(
    private: RsaPrivateKey,
    server_stream: DuplexStream,
    finished: Arc<AtomicBool>,
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
            let body = decrypted.body.clone();
            // The client wraps every call in `invokeWithLayer`, so peek at the
            // nested method id (after the outer id and the layer field).
            let inner = {
                let mut reader = rtd_bin::Reader::new(&body);
                reader.take_u32().ok();
                reader.take_i32().ok();
                reader.peek_id().unwrap_or(0)
            };
            let payload = if inner == rtd_tg::AccountInitTakeoutSession::TYPE_ID {
                let mut writer = Writer::new();
                Object::AccountTakeout(Box::new(AccountTakeout { id: 7 })).encode(&mut writer);
                writer.into_vec()
            } else if inner == rtd_tg::AccountFinishTakeoutSession::TYPE_ID {
                finished.store(true, Ordering::SeqCst);
                let mut writer = Writer::new();
                Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut writer);
                writer.into_vec()
            } else {
                let mut writer = Writer::new();
                Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut writer);
                writer.into_vec()
            };
            let reply = rtd_proto::RpcResult::new(decrypted.header.msg_id, payload).encode();
            server_msg_id += 1;
            let header = MessageHeader {
                salt: server_salt,
                session_id: decrypted.header.session_id,
                msg_id: server_msg_id,
                seq_no: decrypted.header.seq_no,
            };
            let reply = encrypt(&auth_key, &header, &reply, Side::Server, &mut random).encode();
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
fn takeout_wraps_calls_and_finishes_on_success() {
    let (private, public) = keypair(41);
    let (client_stream, server_stream) = DuplexStream::pair();
    let finished = Arc::new(AtomicBool::new(false));
    let server = spawn_takeout_server(private, server_stream, finished.clone());

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");

    let result = takeout::run(&mut client, TakeoutConfig::all(), |client, takeout_id| {
        assert_eq!(takeout_id, 7);
        let query = takeout::wrap(Method::HelpGetConfig(Box::new(HelpGetConfig)), takeout_id);
        client.invoke(query)
    });
    let object = result.expect("takeout run");
    assert!(matches!(object, Object::Bool(_)));
    assert!(finished.load(Ordering::SeqCst), "session must be finished");

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn takeout_finishes_on_closure_error() {
    let (private, public) = keypair(43);
    let (client_stream, server_stream) = DuplexStream::pair();
    let finished = Arc::new(AtomicBool::new(false));
    let server = spawn_takeout_server(private, server_stream, finished.clone());

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");

    let result: Result<(), Error> =
        takeout::run(&mut client, TakeoutConfig::all(), |_client, _takeout_id| {
            Err(Error::Unexpected("closure failed".to_owned()))
        });
    assert!(result.is_err());
    assert!(
        finished.load(Ordering::SeqCst),
        "session must be finished even on error"
    );

    drop(client);
    server.join().expect("server thread");
}
