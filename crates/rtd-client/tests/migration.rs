//! Datacenter routing across a `*_MIGRATE_*` error.

use std::collections::BTreeMap;
use std::sync::Arc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_bin::Writer;
use rtd_client::{Client, DcRouter, DuplexStream, StaticResolver, Transport};
use rtd_crypto::{EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{
    ClientConfig, ServerConfig, Stream as ExchangeStream, TestServerRng, server_exchange,
};
use rtd_mt::{Object as MtObject, RpcError};
use rtd_tg::{Bool, HelpGetConfig, Method, Object};
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

/// Runs the exchange, then answers every request by encoding `reply(())` once.
fn spawn_server(
    private: RsaPrivateKey,
    server_stream: DuplexStream,
    body: Vec<u8>,
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
                strict_dc: false,
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

fn config(dc: i32, public: rsa::RsaPublicKey) -> ClientConfig {
    ClientConfig {
        dc,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public],
    }
}

fn clock() -> Arc<dyn Clock + Send + Sync> {
    Arc::new(SystemClock)
}

fn encode_mt(object: &MtObject) -> Vec<u8> {
    let mut writer = Writer::new();
    object.encode(&mut writer);
    writer.into_vec()
}

#[test]
fn router_follows_a_migrate_error() {
    // DC 2 answers a PHONE_MIGRATE_5 error; DC 5 answers Bool.
    let (private2, public2) = keypair(101);
    let (client2, server2) = DuplexStream::pair();
    let server2 = spawn_server(
        private2,
        server2,
        encode_mt(&MtObject::RpcError(Box::new(RpcError::Rpc_error {
            error_code: 303,
            error_message: "PHONE_MIGRATE_5".to_owned(),
        }))),
    );

    let (private5, public5) = keypair(103);
    let (client5, server5) = DuplexStream::pair();
    let mut bool_writer = Writer::new();
    Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut bool_writer);
    let server5 = spawn_server(private5, server5, bool_writer.into_vec());

    let mut streams = BTreeMap::new();
    streams.insert(2, (config(2, public2), client2));
    streams.insert(5, (config(5, public5), client5));

    let connect = move |dc: i32| {
        let (config, stream) = streams
            .remove(&dc)
            .ok_or_else(|| rtd_client::Error::Unexpected(format!("no connection for DC {dc}")))?;
        Client::connect(stream, &config, clock())
    };

    let mut router = DcRouter::new(2, StaticResolver::production(), connect).expect("router");
    assert_eq!(router.current(), 2);

    let object = router
        .invoke(Method::HelpGetConfig(Box::new(HelpGetConfig)))
        .expect("migrated call");
    assert!(matches!(object, Object::Bool(_)));
    assert_eq!(router.current(), 5);

    drop(router);
    server2.join().expect("dc2 server");
    server5.join().expect("dc5 server");
}

#[test]
fn reconnecting_client_recovers_after_a_missed_pong() {
    use std::time::Duration;

    use rtd_client::{Keepalive, ReconnectingClient};

    // Two servers: the initial one and the replacement after reconnect.
    let (private_a, public_a) = keypair(107);
    let (client_a, server_a) = DuplexStream::pair();
    let server_a = spawn_server(private_a, server_a, {
        let mut writer = Writer::new();
        Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut writer);
        writer.into_vec()
    });
    let (private_b, public_b) = keypair(109);
    let (client_b, server_b) = DuplexStream::pair();
    let server_b = spawn_server(private_b, server_b, {
        let mut writer = Writer::new();
        Object::Bool(Box::new(Bool::BoolTrue)).encode(&mut writer);
        writer.into_vec()
    });

    let initial = Client::connect(client_a, &config(2, public_a), clock()).expect("connect a");
    let session = initial.session_data();
    let replacement = Client::connect(client_b, &config(2, public_b), clock()).expect("connect b");
    let mut slot = Some(replacement);

    // The factory returns the pre-connected replacement exactly once.
    let connect = move || {
        slot.take()
            .ok_or_else(|| rtd_client::Error::Unexpected("no replacement".to_owned()))
    };

    let keepalive = Keepalive::new(Duration::from_secs(60), Duration::from_secs(5), 0);
    let mut client = ReconnectingClient::new(
        initial,
        session,
        connect,
        keepalive,
        Duration::from_millis(100),
        Duration::from_secs(1),
    );

    // Send a ping and let it lapse.
    assert_eq!(client.ping_due(60_000, 1), Some(1));
    client.ping_sent(1, 60_000);
    assert!(client.maintain(100_000).expect("maintain reconnects"));

    // The queued call now completes over the new connection.
    let object = client
        .invoke(Method::HelpGetConfig(Box::new(HelpGetConfig)))
        .expect("call after reconnect");
    assert!(matches!(object, Object::Bool(_)));

    drop(client);
    server_a.join().expect("server a");
    server_b.join().expect("server b");
}

#[test]
fn request_call_returns_the_phone_call() {
    use rtd_client::default_protocol;
    use rtd_tg::{InputUser, PhoneCall, PhonePhoneCall};

    let call = PhoneCall::PhoneCallRequested {
        flags: 0,
        video: false,
        id: 5,
        access_hash: 6,
        date: 0,
        admin_id: 1,
        participant_id: 2,
        g_a_hash: vec![1, 2, 3],
        protocol: Box::new(default_protocol()),
    };
    let mut writer = Writer::new();
    Object::PhonePhoneCall(Box::new(PhonePhoneCall {
        phone_call: Box::new(call),
        users: Vec::new(),
    }))
    .encode(&mut writer);

    let (private, public) = keypair(113);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream, writer.into_vec());

    let mut client =
        Client::connect(client_stream, &config(2, public), clock()).expect("client exchange");
    let result = rtd_client::request_call(
        &mut client,
        InputUser::InputUser {
            user_id: 7,
            access_hash: 8,
        },
        1,
        vec![9, 9],
        false,
    )
    .expect("request call");
    assert!(matches!(
        result.phone_call.as_ref(),
        PhoneCall::PhoneCallRequested { id, .. } if *id == 5
    ));
    drop(client);
    server.join().expect("server thread");
}
