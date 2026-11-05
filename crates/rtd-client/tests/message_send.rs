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
use rtd_tg::{DecodeLimits, Decoder, InputPeer, Method, Object, Updates, UsersGetUsers};
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
        UsersGetUsers::TYPE_ID => {
            let mut writer = Writer::new();
            writer.put_u32(0x1cb5_c415);
            writer.put_u32(1);
            Object::User(Box::new(self_user_value())).encode(&mut writer);
            writer.into_vec()
        }
        rtd_tg::AuthSignIn::TYPE_ID => encode(&Object::AuthAuthorization(Box::new(
            rtd_tg::AuthAuthorization::Authorization {
                flags: 0,
                setup_password_required: false,
                otherwise_relogin_days: None,
                tmp_sessions: None,
                future_auth_token: None,
                user: Box::new(self_user_value()),
            },
        ))),
        _ => encode(&Object::Bool(Box::new(rtd_tg::Bool::BoolTrue))),
    })
}

/// A minimal `User` flagged as the current account.
fn self_user_value() -> rtd_tg::User {
    rtd_tg::User::User {
        flags: 0,
        self_: true,
        contact: false,
        mutual_contact: false,
        deleted: false,
        bot: false,
        bot_chat_history: false,
        bot_nochats: false,
        verified: false,
        restricted: false,
        min: false,
        bot_inline_geo: false,
        support: false,
        scam: false,
        apply_min_photo: false,
        fake: false,
        bot_attach_menu: false,
        premium: false,
        attach_menu_enabled: false,
        flags2: 0,
        bot_can_edit: false,
        close_friend: false,
        stories_hidden: false,
        stories_unavailable: false,
        contact_require_premium: false,
        bot_business: false,
        bot_has_main_app: false,
        bot_forum_view: false,
        bot_forum_can_manage_topics: false,
        bot_can_manage_bots: false,
        bot_guestchat: false,
        bot_guard: false,
        id: 7,
        access_hash: Some(9),
        first_name: Some("Alice".to_owned()),
        last_name: None,
        username: Some("alice".to_owned()),
        phone: None,
        photo: None,
        status: None,
        bot_info_version: None,
        restriction_reason: None,
        bot_inline_placeholder: None,
        lang_code: None,
        emoji_status: None,
        usernames: None,
        stories_max_id: None,
        color: None,
        profile_color: None,
        bot_active_users: None,
        bot_verification_icon: None,
        send_paid_messages_stars: None,
        linked_community_id: None,
    }
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

#[test]
fn invoke_json_round_trips() {
    use rtd_client::{JsonValue, encode_method};
    use rtd_tg::{HelpGetConfig, Method};

    let (private, public) = keypair(127);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");

    let request = encode_method(&Method::HelpGetConfig(Box::new(HelpGetConfig)));
    let response = client.invoke_json(&request).expect("invoke_json");
    match &response {
        JsonValue::Object(entries) => assert_eq!(
            entries.get("@type"),
            Some(&JsonValue::String("boolTrue".to_owned()))
        ),
        other => panic!("unexpected response: {other:?}"),
    }

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn invoke_raw_decodes_a_vector() {
    use rtd_client::InvokeResult;
    use rtd_tg::{InputUser, UsersGetUsers};

    let (private, public) = keypair(131);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");

    let method = Method::UsersGetUsers(Box::new(UsersGetUsers {
        id: vec![InputUser::InputUserSelf],
    }));
    match client.invoke_raw(method).expect("invoke_raw") {
        InvokeResult::Vector(items) => assert_eq!(items.len(), 1),
        other => panic!("expected a vector, got {other:?}"),
    }

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn self_user_returns_the_self_user() {
    let (private, public) = keypair(137);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");

    match client.self_user().expect("self_user") {
        rtd_tg::User::User {
            self_: true, id: 7, ..
        } => {}
        other => panic!("unexpected self user: {other:?}"),
    }

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn invoke_raw_json_round_trips() {
    use rtd_client::encode_method;
    use rtd_tg::{HelpGetConfig, Method};

    let (private, public) = keypair(139);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");

    let request = encode_method(&Method::HelpGetConfig(Box::new(HelpGetConfig))).to_string();
    let response = client.invoke_raw_json(&request).expect("invoke_raw_json");
    assert!(
        response.contains("boolTrue"),
        "unexpected response: {response}"
    );

    drop(client);
    server.join().expect("server thread");
}

#[test]
fn flow_reports_status_and_signs_in() {
    use rtd_client::{ConstantPassword, Flow, Status, default_code_settings};
    use rtd_tg::{AuthSentCode, AuthSentCodeType};

    let (private, public) = keypair(149);
    let (client_stream, server_stream) = DuplexStream::pair();
    let server = spawn_server(private, server_stream);

    let mut client =
        Client::connect(client_stream, &config(public), clock()).expect("client exchange");

    let flow = Flow::new(1, "hash", "+10000000000", default_code_settings());
    assert_eq!(
        flow.status(&mut client).expect("status"),
        Status::Authorized
    );

    let sent = AuthSentCode::SentCode {
        flags: 0,
        type_: Box::new(AuthSentCodeType::SentCodeTypeApp { length: 5 }),
        phone_code_hash: "hash".to_owned(),
        next_type: None,
        timeout: None,
    };
    let authorization = flow
        .sign_in(&mut client, &sent, "12345", &ConstantPassword::new("pw"))
        .expect("sign_in");
    assert!(matches!(
        authorization,
        rtd_tg::AuthAuthorization::Authorization { .. }
    ));

    drop(client);
    server.join().expect("server thread");
}
