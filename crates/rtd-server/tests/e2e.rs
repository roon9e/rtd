//! In-process end-to-end session tests (tasks 12.1 and 12.2).
//!
//! Each test starts a real [`Server`] on an ephemeral loopback port inside a
//! background tokio runtime thread, then drives a *synchronous* client over one
//! connection: the authorization-key exchange followed by an encrypted built-in
//! RPC. There is one case per plain TCP framing (12.1) and one each for
//! obfuscated2, FakeTLS and WebSocket (12.2).
//!
//! The server is configured with an in-process [`LocalCore`] and an
//! [`Exchange`] closure over a generated RSA key, so the whole path below the
//! socket is exercised without a remote core.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use rsa::{RsaPrivateKey, RsaPublicKey};

use rtd_bin::Writer;
use rtd_crypto::{
    AuthKey, EncryptedMessage, MessageHeader, RandomSource, Side, SystemRandom, decrypt, encrypt,
};
use rtd_exchange::{
    ClientConfig, Stream as ExchangeStream, TestServerRng, client_exchange, server_exchange,
};
use rtd_proto::codec::{Codec, CodecKind, INTERMEDIATE_HEADER};
use rtd_proto::{MessageClass, MessageType, RPC_RESULT_TYPE_ID, RpcResult, Session};
use rtd_server::actor::{BudgetLimits, Exchange, Server, ServerOptions};
use rtd_server::core::{Core, LocalCore};
use rtd_server::ingress::IngressConfig;
use rtd_server::rpc::{DataCenter, ServerConfig};
use rtd_tg::{HelpGetConfig, HelpGetNearestDc, Method, Object};
use rtd_tlprofile::{CANONICAL_LAYER, Limits, Profile, decode_object};
use rtd_transport::faketls::FakeTls;
use rtd_transport::{Obfuscated2, Secret};
use rtd_util::{Clock, SystemClock};

use tokio_tungstenite::tungstenite::{Message as WsMessage, WebSocket};

/// The datacenter served by every test.
const DC: i32 = 2;

/// The plain obfuscated2 MTProxy secret used by the obfuscated2 case.
const OBFUSCATED_SECRET: &str = "52a493bdfb90eea55739eabff2d92a14";
/// The FakeTLS MTProxy secret used by the FakeTLS case.
const FAKETLS_SECRET: &str = "ee852380f362a09343efb4690c4e17862e676f6f676c652e636f6d";

/// Returns a process-wide RSA key pair, generated once.
///
/// RSA-2048 generation dominates the runtime, so all cases share one key via a
/// [`OnceLock`].
fn shared_key() -> &'static (RsaPrivateKey, RsaPublicKey) {
    static KEY: OnceLock<(RsaPrivateKey, RsaPublicKey)> = OnceLock::new();
    KEY.get_or_init(|| {
        let private = RsaPrivateKey::new(&mut rand::thread_rng(), 2048).expect("generate RSA key");
        let public = private.to_public_key();
        (private, public)
    })
}

/// Builds the in-process core with the built-in methods configured.
fn make_core() -> Arc<dyn Core> {
    let config = ServerConfig::new(DC, vec![DataCenter::new(DC, "127.0.0.1", 443)]);
    Arc::new(LocalCore::with_builtins(&config))
}

/// Builds the server options, including the real key-exchange closure.
fn make_options() -> ServerOptions {
    let key = Arc::new(shared_key().0.clone());
    let exchange: Arc<dyn Exchange> = Arc::new(
        move |stream: &mut dyn ExchangeStream,
              clock: &dyn Clock,
              random: &mut dyn RandomSource|
              -> rtd_exchange::Result<rtd_exchange::ServerExchangeResult> {
            let mut bridge = BridgeStream(stream);
            let mut source = BridgeRandom(random);
            let mut parameters = TestServerRng;
            server_exchange(
                &mut bridge,
                &rtd_exchange::ServerConfig {
                    dc: DC,
                    key: key.as_ref(),
                    strict_dc: true,
                },
                &mut parameters,
                clock,
                &mut source,
            )
        },
    );
    ServerOptions {
        dc: DC,
        clock: Arc::new(SystemClock),
        exchange,
        layer: Profile::new(CANONICAL_LAYER),
        idle_timeout: Duration::from_secs(300),
        budget_limits: BudgetLimits::default(),
    }
}

/// A [`Server`] running in a background tokio runtime thread.
///
/// [`TestServer::drop`] stops the runtime and joins the thread, so a test never
/// leaks a listener between cases.
struct TestServer {
    addr: SocketAddr,
    shutdown: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Binds a server on `127.0.0.1:0` and starts serving, returning its address.
    fn start(ingress: IngressConfig) -> Self {
        let (addr_tx, addr_rx) = channel::<SocketAddr>();
        let (shutdown_tx, shutdown_rx) = channel::<()>();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime");
            runtime.block_on(async move {
                let server = Server::bind("127.0.0.1:0", ingress, make_core(), make_options())
                    .await
                    .expect("bind server");
                addr_tx
                    .send(server.local_addr().expect("server address"))
                    .expect("send server address");
                let serving = tokio::spawn(server.serve());
                // Park the async task until `Drop` asks the runtime to stop.
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = shutdown_rx.recv();
                })
                .await;
                serving.abort();
            });
        });
        let addr = addr_rx.recv().expect("receive server address");
        Self {
            addr,
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Adapts a `dyn` exchange stream so the concrete `server_exchange` can consume it.
struct BridgeStream<'a>(&'a mut dyn ExchangeStream);

impl ExchangeStream for BridgeStream<'_> {
    fn send(&mut self, frame: &[u8]) -> rtd_exchange::Result<()> {
        self.0.send(frame)
    }

    fn recv(&mut self) -> rtd_exchange::Result<Vec<u8>> {
        self.0.recv()
    }
}

/// Adapts a `dyn` random source to the sized generic `server_exchange` expects.
struct BridgeRandom<'a>(&'a mut dyn RandomSource);

impl RandomSource for BridgeRandom<'_> {
    fn fill(&mut self, dest: &mut [u8]) {
        self.0.fill(dest);
    }
}

/// Converts any displayable framing error into a stream error.
fn stream_error(message: impl std::fmt::Display) -> rtd_exchange::Error {
    rtd_exchange::Error::Stream(message.to_string())
}

/// A synchronous client transport: codec framing over an owned byte stream.
///
/// `send_header` selects whether the codec's protocol header is written before
/// the first message. Plain TCP and WebSocket write it; the MTProxy transports
/// carry the framing in the obfuscated2 protocol tag instead.
struct ClientStream<S> {
    io: S,
    codec: Box<dyn Codec>,
    send_header: bool,
    header_written: bool,
}

impl<S: Read + Write> ClientStream<S> {
    fn new(io: S, codec: Box<dyn Codec>, send_header: bool) -> Self {
        Self {
            io,
            codec,
            send_header,
            header_written: false,
        }
    }
}

impl<S: Read + Write> ExchangeStream for ClientStream<S> {
    fn send(&mut self, frame: &[u8]) -> rtd_exchange::Result<()> {
        if self.send_header && !self.header_written {
            self.codec
                .write_header(&mut self.io)
                .map_err(stream_error)?;
            self.header_written = true;
        }
        let mut buffer = Vec::with_capacity(frame.len() + 16);
        self.codec
            .write_message(&mut buffer, frame)
            .map_err(stream_error)?;
        self.io.write_all(&buffer).map_err(stream_error)?;
        self.io.flush().map_err(stream_error)?;
        Ok(())
    }

    fn recv(&mut self) -> rtd_exchange::Result<Vec<u8>> {
        self.codec.read_message(&mut self.io).map_err(stream_error)
    }
}

/// Presents a blocking WebSocket as a byte stream.
///
/// Reads pull binary/text messages and buffer their payload; writes accumulate
/// and are sent as one binary message per flush, matching the server's
/// message-per-frame handling.
struct WsStream {
    socket: WebSocket<TcpStream>,
    read_buf: Vec<u8>,
    read_pos: usize,
    write_buf: Vec<u8>,
}

impl WsStream {
    fn new(socket: WebSocket<TcpStream>) -> Self {
        Self {
            socket,
            read_buf: Vec::new(),
            read_pos: 0,
            write_buf: Vec::new(),
        }
    }
}

impl Read for WsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.read_pos < self.read_buf.len() {
                let n = (self.read_buf.len() - self.read_pos).min(buf.len());
                buf[..n].copy_from_slice(&self.read_buf[self.read_pos..self.read_pos + n]);
                self.read_pos += n;
                if self.read_pos == self.read_buf.len() {
                    self.read_buf.clear();
                    self.read_pos = 0;
                }
                return Ok(n);
            }
            if buf.is_empty() {
                return Ok(0);
            }
            match self.socket.read() {
                Ok(WsMessage::Binary(data)) => {
                    self.read_buf = data;
                    self.read_pos = 0;
                }
                Ok(WsMessage::Text(text)) => {
                    self.read_buf = text.into_bytes();
                    self.read_pos = 0;
                }
                Ok(WsMessage::Close(_)) => return Ok(0),
                Ok(_) => {}
                Err(error) => return Err(io::Error::other(error)),
            }
        }
    }
}

impl Write for WsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.write_buf.is_empty() {
            let data = std::mem::take(&mut self.write_buf);
            self.socket
                .send(WsMessage::Binary(data))
                .map_err(io::Error::other)?;
        }
        self.socket.flush().map_err(io::Error::other)
    }
}

/// A synchronous, encrypted session over a completed key exchange.
struct TestClient<S: ExchangeStream> {
    stream: S,
    key: AuthKey,
    session: Session,
    random: SystemRandom,
}

impl<S: ExchangeStream> TestClient<S> {
    /// Opens a session with the exchanged key and salt.
    fn connect(stream: S, result: rtd_exchange::ClientExchangeResult) -> Self {
        let mut session = Session::with_id(Arc::new(SystemClock), result.session_id);
        session.set_salt(result.server_salt);
        Self {
            stream,
            key: result.auth_key,
            session,
            random: SystemRandom,
        }
    }

    /// Sends one encrypted content request and returns the raw result bytes.
    fn call(&mut self, method: &Method) -> Vec<u8> {
        let mut writer = Writer::new();
        method.encode(&mut writer);
        let body = writer.into_vec();

        let sequence = self
            .session
            .allocate(MessageType::FromClient, MessageClass::Content);
        let header = MessageHeader {
            salt: self.session.salt(),
            session_id: self.session.session_id(),
            msg_id: sequence.msg_id,
            seq_no: sequence.seq_no,
        };
        let frame = encrypt(&self.key, &header, &body, Side::Client, &mut self.random).encode();
        self.stream.send(&frame).expect("send request");

        loop {
            let wire = self.stream.recv().expect("receive response frame");
            let message = EncryptedMessage::decode(&wire).expect("decode encrypted frame");
            let decrypted =
                decrypt(&self.key, &message, Side::Server).expect("decrypt server frame");
            // The server may interleave `new_session_created`, `msgs_ack` or a
            // `pong`; skip everything until the RPC result.
            if peek_id(&decrypted.body) == RPC_RESULT_TYPE_ID {
                let result = RpcResult::decode(&decrypted.body).expect("decode rpc_result");
                assert_eq!(result.request_message_id, sequence.msg_id);
                return result.result;
            }
        }
    }
}

/// Returns the little-endian constructor identifier at the start of `body`.
fn peek_id(body: &[u8]) -> u32 {
    body.get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .unwrap_or(0)
}

/// Completes an exchange and one RPC over `stream`, returning the decoded result.
fn run_session<S: ExchangeStream>(mut stream: S, method: &Method) -> Object {
    let config = ClientConfig {
        dc: DC,
        temporary: false,
        expires_in: 0,
        public_keys: vec![shared_key().1.clone()],
    };
    let clock = SystemClock;
    let mut random = SystemRandom;
    let exchange =
        client_exchange(&mut stream, &config, &clock, &mut random).expect("client exchange");
    let mut client = TestClient::connect(stream, exchange);
    let bytes = client.call(method);
    decode_object(Profile::new(CANONICAL_LAYER), &bytes, Limits::default())
        .expect("decode built-in result")
}

/// The plain `help.getConfig` request.
fn get_config() -> Method {
    Method::HelpGetConfig(Box::new(HelpGetConfig))
}

/// The plain `help.getNearestDc` request.
fn get_nearest_dc() -> Method {
    Method::HelpGetNearestDc(Box::new(HelpGetNearestDc))
}

/// Asserts the decoded result is the configured `help.getConfig` value.
fn assert_config(object: &Object) {
    let Object::Config(config) = object else {
        panic!("expected help.getConfig to return Config, got {object:?}");
    };
    assert_eq!(config.this_dc, DC);
    assert_eq!(config.dc_options.len(), 1);
    assert_eq!(config.dc_options[0].id, DC);
}

/// Asserts the decoded result is the serving `help.getNearestDc` value.
fn assert_nearest_dc(object: &Object) {
    let Object::NearestDc(nearest) = object else {
        panic!("expected help.getNearestDc to return NearestDc, got {object:?}");
    };
    assert_eq!(nearest.this_dc, DC);
    assert_eq!(nearest.nearest_dc, DC);
}

/// Runs one plain-TCP framing through the full session and asserts the result.
fn plain_framing_session(kind: CodecKind) {
    let server = TestServer::start(IngressConfig::new());
    let tcp = TcpStream::connect(server.addr).expect("connect");
    let stream = ClientStream::new(tcp, kind.build(), true);
    let object = run_session(stream, &get_config());
    assert_config(&object);
}

#[test]
fn plain_abridged_full_session() {
    plain_framing_session(CodecKind::Abridged);
}

#[test]
fn plain_intermediate_full_session() {
    plain_framing_session(CodecKind::Intermediate);
}

#[test]
fn plain_padded_intermediate_full_session() {
    plain_framing_session(CodecKind::PaddedIntermediate);
}

#[test]
fn plain_full_full_session() {
    plain_framing_session(CodecKind::Full);
}

#[test]
fn mtproxy_obfuscated2_full_session() {
    let secret = Secret::parse(OBFUSCATED_SECRET).expect("parse secret");
    let server = TestServer::start(IngressConfig::new().with_secret(secret.clone()));
    let key = secret.obfuscated2_key().expect("obfuscated2 key").to_vec();
    let tcp = TcpStream::connect(server.addr).expect("connect");
    let obfuscated =
        Obfuscated2::handshake(tcp, INTERMEDIATE_HEADER, DC as u16, &key).expect("obfuscated2");
    let stream = ClientStream::new(obfuscated, CodecKind::Intermediate.build(), false);
    let object = run_session(stream, &get_nearest_dc());
    assert_nearest_dc(&object);
}

#[test]
fn mtproxy_faketls_full_session() {
    let secret = Secret::parse(FAKETLS_SECRET).expect("parse secret");
    let server = TestServer::start(IngressConfig::new().with_secret(secret.clone()));
    let key = secret.faketls_key().expect("faketls key").to_vec();
    let cloak = secret.cloak_host().to_owned();
    let tcp = TcpStream::connect(server.addr).expect("connect");
    let mut tls = FakeTls::new(tcp);
    tls.handshake(&key, &cloak).expect("faketls handshake");
    let obfuscated =
        Obfuscated2::handshake(tls, INTERMEDIATE_HEADER, DC as u16, &key).expect("obfuscated2");
    let stream = ClientStream::new(obfuscated, CodecKind::Intermediate.build(), false);
    let object = run_session(stream, &get_nearest_dc());
    assert_nearest_dc(&object);
}

#[test]
fn websocket_full_session() {
    let server = TestServer::start(IngressConfig::new().with_websocket(true));
    let tcp = TcpStream::connect(server.addr).expect("connect");
    let request = format!("ws://127.0.0.1:{}/apiws", server.addr.port());
    let (socket, _response) =
        tokio_tungstenite::tungstenite::client(request, tcp).expect("websocket handshake");
    let stream = ClientStream::new(WsStream::new(socket), CodecKind::Abridged.build(), true);
    let object = run_session(stream, &get_config());
    assert_config(&object);
}

/// Adapts the `rtd-client` WebSocket transport to the exchange stream interface.
struct TransportStream(rtd_client::TcpTransport);

impl ExchangeStream for TransportStream {
    fn send(&mut self, frame: &[u8]) -> rtd_exchange::Result<()> {
        rtd_client::WriteTransport::send_frame(&mut self.0, frame).map_err(stream_error)
    }

    fn recv(&mut self) -> rtd_exchange::Result<Vec<u8>> {
        rtd_client::ReadTransport::recv_frame(&mut self.0).map_err(stream_error)
    }
}

/// Drives the *client's own* WebSocket transport through the full session.
#[test]
fn websocket_client_transport_full_session() {
    let server = TestServer::start(IngressConfig::new().with_websocket(true));
    let address = format!("127.0.0.1:{}", server.addr.port());
    let transport =
        rtd_client::connect_websocket(&address, &address, "/apiws").expect("websocket dial");
    let object = run_session(TransportStream(transport), &get_config());
    assert_config(&object);
}
