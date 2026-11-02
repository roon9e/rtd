//! In-band update delivery: a server message that is not the answer to a request
//! is offered to the update handler.

use std::sync::{Arc, Mutex};

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_bin::Writer;
use rtd_client::{Client, DuplexStream, InitParams, Transport};
use rtd_crypto::{AuthKey, EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_tg::{Bool, Object, Update, Updates};
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

/// The server side of the encrypted session.
struct Peer {
    auth_key: AuthKey,
    salt: i64,
    random: SystemRandom,
    msg_id: i64,
}

impl Peer {
    /// Encrypts and sends one server message.
    fn send(&mut self, stream: &mut ServerStream, session_id: i64, seq_no: i32, body: Vec<u8>) {
        self.msg_id += 1;
        let header = MessageHeader {
            salt: self.salt,
            session_id,
            msg_id: self.msg_id,
            seq_no,
        };
        let frame = encrypt(
            &self.auth_key,
            &header,
            &body,
            Side::Server,
            &mut self.random,
        )
        .encode();
        stream.0.send_frame(&frame).expect("send");
    }
}

/// Runs the server exchange, sends `preamble` (if any) before answering the first
/// request, and answers every request with a `Bool`.
fn spawn_server(
    private: RsaPrivateKey,
    server_stream: DuplexStream,
    preamble: Option<Vec<u8>>,
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
        let mut peer = Peer {
            auth_key: result.auth_key,
            salt: result.server_salt,
            random,
            msg_id: 0,
        };

        let mut answered = false;
        while let Ok(frame) = stream.0.recv_frame() {
            let message = EncryptedMessage::decode(&frame).expect("decode client frame");
            let decrypted =
                decrypt(&peer.auth_key, &message, Side::Client).expect("decrypt client");
            if answered {
                continue;
            }
            let session_id = decrypted.header.session_id;
            if let Some(preamble) = &preamble {
                // A server content message carries an odd sequence number.
                peer.send(&mut stream, session_id, 1, preamble.clone());
            }
            let mut writer = Writer::new();
            Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut writer);
            let body =
                rtd_proto::RpcResult::new(decrypted.header.msg_id, writer.into_vec()).encode();
            peer.send(&mut stream, session_id, decrypted.header.seq_no, body);
            answered = true;
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

/// Encodes a body from a canonical object.
fn encode(object: &Object) -> Vec<u8> {
    let mut writer = Writer::new();
    object.encode(&mut writer);
    writer.into_vec()
}

#[test]
fn an_in_band_update_is_delivered() {
    let (private, public) = keypair(41);
    let (client_stream, server_stream) = DuplexStream::pair();
    let update = Updates::UpdateShort {
        update: Box::new(Update::UpdateConfig),
        date: 0,
    };
    let mut preamble = Writer::new();
    update.encode(&mut preamble);
    let server = spawn_server(private, server_stream, Some(preamble.into_vec()));

    let mut client = Client::connect(client_stream, &config(public), clock()).expect("connect");
    let seen = Arc::new(Mutex::new(Vec::<Updates>::new()));
    let sink = Arc::clone(&seen);
    client.set_update_handler(move |updates| sink.lock().expect("lock").push(updates));

    let result = client
        .initialize(params())
        .expect("initialize returns a result");
    assert!(matches!(result, Object::Bool(_)));

    let seen = seen.lock().expect("lock");
    assert_eq!(seen.len(), 1, "one update was delivered");
    assert!(matches!(seen[0], Updates::UpdateShort { .. }));
    drop(seen);

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn an_unknown_message_does_not_disturb_the_result() {
    let (private, public) = keypair(43);
    let (client_stream, server_stream) = DuplexStream::pair();
    // A `Bool` is neither an `Updates` nor an `Update`; the handler must not fire
    // and the awaited result must still arrive.
    let server = spawn_server(
        private,
        server_stream,
        Some(encode(&Object::Bool(Box::new(Bool::BoolFalse)))),
    );

    let mut client = Client::connect(client_stream, &config(public), clock()).expect("connect");
    let calls = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&calls);
    client.set_update_handler(move |_updates| *sink.lock().expect("lock") += 1);

    let result = client
        .initialize(params())
        .expect("initialize returns a result");
    let Object::Bool(value) = result else {
        panic!("expected a Bool result");
    };
    assert!(matches!(*value, Bool::BoolTrue));
    assert_eq!(*calls.lock().expect("lock"), 0, "the handler did not fire");

    drop(client);
    server.join().expect("server thread");
}
