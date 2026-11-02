//! Client round trip against an in-process server: the auth-key exchange, an
//! encrypted `invokeWithLayer`/`initConnection` request, and `rpc_result`
//! correlation, all over an in-memory duplex.

use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_bin::Writer;
use rtd_client::{Client, DuplexStream, InitParams, Transport};
use rtd_crypto::{EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_tg::{Bool, Object};
use rtd_util::SystemClock;

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

#[test]
fn client_completes_exchange_and_invokes() {
    let mut rng = StdRng::seed_from_u64(19);
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
    let public = private.to_public_key();

    let (client_stream, server_stream) = DuplexStream::pair();

    let server = std::thread::spawn(move || {
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

        // Reply once to the client's first content request with a known object.
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
    });

    let clock: Arc<dyn rtd_util::Clock + Send + Sync> = Arc::new(SystemClock);
    let config = ClientConfig {
        dc: 2,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public],
    };
    let mut client =
        Client::connect(client_stream, &config, clock).expect("client exchange completes");

    let result = client
        .initialize(InitParams {
            api_id: 2040,
            device_model: "rtd".to_owned(),
            system_version: "test".to_owned(),
            app_version: "0.0.0".to_owned(),
            system_lang_code: "en".to_owned(),
            lang_pack: String::new(),
            lang_code: "en".to_owned(),
        })
        .expect("initialize returns the server's result");
    assert!(
        matches!(result, Object::Bool(_)),
        "expected the server's Bool result, got {result:?}"
    );

    drop(client);
    server.join().expect("server thread");
}
