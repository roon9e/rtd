//! File download round trip against an in-process server.

use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;
use rtd_bin::Writer;
use rtd_client::{Client, Downloader, DuplexStream, Transport};
use rtd_crypto::{EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_tg::{
    DecodeLimits, Decoder, InputFileLocation, Method, Object, StorageFileType, UploadFile,
};
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

/// Serves `payload` in `upload.getFile` ranges.
fn spawn_server(
    private: RsaPrivateKey,
    server_stream: DuplexStream,
    payload: Vec<u8>,
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
            let body = {
                let mut decoder = Decoder::new(&decrypted.body, DecodeLimits::default());
                Method::decode(&mut decoder)
            };
            let result_body = match body {
                Ok(method) => match unwrap_method(&method) {
                    Method::UploadGetFile(request) => {
                        let start = usize::try_from(request.offset)
                            .unwrap_or(0)
                            .min(payload.len());
                        let end = start
                            .saturating_add(usize::try_from(request.limit.max(0)).unwrap_or(0))
                            .min(payload.len());
                        encode(&Object::UploadFile(Box::new(UploadFile::File {
                            type_: Box::new(StorageFileType::FileUnknown),
                            mtime: 0,
                            bytes: payload[start..end].to_vec(),
                        })))
                    }
                    _ => encode(&Object::Bool(Box::new(rtd_tg::Bool::BoolTrue))),
                },
                Err(_) => encode(&Object::Bool(Box::new(rtd_tg::Bool::BoolTrue))),
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
fn downloads_a_file_intact_in_parts() {
    let (private, public) = keypair(97);
    let (client_stream, server_stream) = DuplexStream::pair();
    let payload: Vec<u8> = (0..(20 * 1024u32)).map(|i| (i % 251) as u8).collect();
    let server = spawn_server(private, server_stream, payload.clone());

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");
    let location = InputFileLocation::InputFileLocation {
        volume_id: 0,
        local_id: 0,
        secret: 0,
        file_reference: Vec::new(),
    };
    let downloader = Downloader::new().with_part_size(4096);
    let downloaded = downloader
        .download(&mut client, location, payload.len())
        .expect("download");
    assert_eq!(downloaded, payload);

    drop(client);
    server.join().expect("server thread");
}
