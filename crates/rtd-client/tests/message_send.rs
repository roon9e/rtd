//! Message-send round trip against an in-process server.

use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_bin::Writer;
use rtd_client::{Client, DuplexStream, RequestBuilder, Transport};
use rtd_crypto::{EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_tg::{DecodeLimits, Decoder, InputPeer, Method, Object, Updates};
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

/// Answers `messages.sendMessage` with an empty `updates` envelope; everything
/// else with a `Bool`.
fn reply(body: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = Decoder::new(body, DecodeLimits::default());
    let method = Method::decode(&mut decoder).ok()?;
    Some(match unwrap_method(&method).type_id() {
        rtd_tg::MessagesSendMessage::TYPE_ID => {
            encode(&Object::Updates(Box::new(Updates::Updates {
                updates: Vec::new(),
                users: Vec::new(),
                chats: Vec::new(),
                date: 0,
                seq: 0,
            })))
        }
        _ => encode(&Object::Bool(Box::new(rtd_tg::Bool::BoolTrue))),
    })
}

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
            let reply_frame =
                encrypt(&auth_key, &header, &result, Side::Server, &mut random).encode();
            if stream.0.send_frame(&reply_frame).is_err() {
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
fn send_message_round_trips() {
    let (private, public) = keypair(71);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");

    let method = RequestBuilder::new(InputPeer::InputPeerEmpty, "hello").build();
    let object = client.invoke(method).expect("send message");
    assert!(matches!(object, Object::Updates(_)));

    drop(client);
    server.join().expect("server thread");
}
