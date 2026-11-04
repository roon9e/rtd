//! Upload progress and cancellation against an in-process server.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_bin::Writer;
use rtd_client::{Client, DuplexStream, Error, Progress, Transport, Uploader};
use rtd_crypto::{EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_tg::{Bool, Object};
use rtd_util::{Clock, SystemClock};

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

/// Answers every request with `Bool`.
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
                strict_dc: true,
            },
            &mut parameters,
            &clock,
            &mut random,
        )
        .expect("server exchange");
        let auth_key = result.auth_key;
        let server_salt = result.server_salt;

        let mut writer = Writer::new();
        Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut writer);
        let body = writer.into_vec();

        let mut server_msg_id = 0i64;
        while let Ok(frame) = stream.0.recv_frame() {
            let message = EncryptedMessage::decode(&frame).expect("decode client frame");
            let decrypted = decrypt(&auth_key, &message, Side::Client).expect("decrypt client");
            let result = rtd_proto::RpcResult::new(decrypted.header.msg_id, body.clone()).encode();
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

/// Records each reported `(uploaded, total)` pair.
#[derive(Default)]
struct Recording {
    calls: Mutex<Vec<(usize, usize)>>,
}

impl Progress for Recording {
    fn progress(&self, uploaded: usize, total: usize) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((uploaded, total));
    }
}

/// Cancels after the first reported part.
#[derive(Default)]
struct CancelAfterFirst {
    calls: AtomicUsize,
}

impl Progress for CancelAfterFirst {
    fn progress(&self, _uploaded: usize, _total: usize) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.calls.load(Ordering::SeqCst) >= 1
    }
}

#[test]
fn progress_is_monotonic() {
    let (private, public) = keypair(79);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");
    let data = vec![7u8; 20 * 1024];
    let progress = Recording::default();
    let uploader = Uploader::new().with_part_size(4096);
    uploader
        .upload_with_progress(&mut client, "file.bin", &data, &progress)
        .expect("upload");

    let calls = progress
        .calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert!(!calls.is_empty());
    let mut previous = 0;
    for (uploaded, total) in calls {
        assert_eq!(total, data.len());
        assert!(uploaded > previous, "progress must strictly increase");
        previous = uploaded;
    }
    assert_eq!(previous, data.len());

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn cancellation_stops_the_upload() {
    let (private, public) = keypair(83);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");
    let data = vec![7u8; 20 * 1024];
    let progress = CancelAfterFirst::default();
    let uploader = Uploader::new().with_part_size(4096);
    let error = uploader
        .upload_with_progress(&mut client, "file.bin", &data, &progress)
        .expect_err("cancelled upload must fail");
    assert!(matches!(error, Error::Unexpected(_)), "{error:?}");

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn concurrent_upload_splits_parts_across_clients() {
    use rtd_client::ConcurrentUploader;

    let mut clients = Vec::new();
    let mut servers = Vec::new();
    for seed in [89u64, 97] {
        let (private, public) = keypair(seed);
        let (client_stream, server_stream) = DuplexStream::pair();
        servers.push(spawn_server(private, server_stream));
        clients.push(
            Client::connect(client_stream, &config(public), clock()).expect("client exchange"),
        );
    }

    let data = vec![5u8; 20 * 1024];
    let uploader = ConcurrentUploader::new().with_part_size(4096);
    let file = uploader
        .upload(&mut clients, "file.bin", &data)
        .expect("concurrent upload");
    match file {
        rtd_tg::InputFile::InputFile { parts, .. } => assert_eq!(parts, 5),
        other => panic!("unexpected input file: {other:?}"),
    }

    drop(clients);
    for server in servers {
        server.join().expect("server thread");
    }
}
