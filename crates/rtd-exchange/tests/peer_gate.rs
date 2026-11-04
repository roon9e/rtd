//! External-peer gate: run the official engine's test server and complete the
//! authorization-key exchange against it over a real socket, then drive a full
//! encrypted session by sending an encrypted `ping` and requiring the peer's
//! matching `pong`.
//!
//! The official engine is GPL-2.0 and is never linked or vendored; this test
//! only *runs* its published binary as a peer. Set `RTD_PEER_TESTSERVER` to the
//! `mtproto-testserver` binary path to enable the gate; without it the test is
//! skipped so the workspace still builds and tests where the peer is absent.

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use rtd_bin::Writer;
use rtd_crypto::{AuthKey, EncryptedMessage, MessageHeader, Side, SystemRandom, decrypt, encrypt};
use rtd_exchange::{ClientConfig, Error, Result, Stream, client_exchange};
use rtd_mt::{
    BadMsgNotification, DecodeLimits, Decoder, MessageContainer, Object, Ping, RpcResult,
};
use rtd_proto::{Codec, Intermediate, MessageClass, MessageSeq, MessageType, Session};
use rtd_transport::Obfuscated2;
use rtd_util::SystemClock;

/// A [`Stream`] over an obfuscated2 TCP connection using intermediate framing.
///
/// The official test server accepts only obfuscated2 on TCP; the framing is
/// carried by the obfuscation protocol tag, so no plain transport header is
/// written. The socket has read/write timeouts so a non-answering peer fails the
/// gate instead of hanging.
struct CodecStream {
    stream: Obfuscated2<TcpStream>,
    codec: Intermediate,
}

impl CodecStream {
    fn connect(address: &str, dc: u16) -> Result<Self> {
        let socket =
            TcpStream::connect(address).map_err(|error| Error::Stream(error.to_string()))?;
        socket
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|error| Error::Stream(error.to_string()))?;
        socket
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|error| Error::Stream(error.to_string()))?;
        let stream = Obfuscated2::handshake(socket, [0xee; 4], dc, &[])
            .map_err(|error| Error::Stream(error.to_string()))?;
        Ok(Self {
            stream,
            codec: Intermediate,
        })
    }
}

impl Stream for CodecStream {
    fn send(&mut self, frame: &[u8]) -> Result<()> {
        self.codec
            .write_message(&mut self.stream, frame)
            .map_err(|error| Error::Stream(error.to_string()))
    }

    fn recv(&mut self) -> Result<Vec<u8>> {
        self.codec
            .read_message(&mut self.stream)
            .map_err(|error| Error::Stream(error.to_string()))
    }
}

/// Keeps the child process alive for the duration of the test.
struct Peer {
    child: Child,
    address: String,
    public_key_pem: String,
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts the official test server and reads its advertised address and key.
fn start_peer(binary: &str) -> Peer {
    let mut child = Command::new(binary)
        .arg("--api")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn mtproto-testserver");
    let stdout = child.stdout.take().expect("child stdout");
    let mut lines = BufReader::new(stdout).lines();
    let line = lines
        .next()
        .expect("test server advertised a line")
        .expect("read advertised line");
    let value: serde_json::Value = serde_json::from_str(&line).expect("advertised JSON");
    let address = value["address"].as_str().expect("address").to_owned();
    // The server escapes newlines in the PEM for its JSON output.
    let public_key_pem = value["public_key_pem"]
        .as_str()
        .expect("public key")
        .replace("\\n", "\n");
    Peer {
        child,
        address,
        public_key_pem,
    }
}

/// A decrypted reply, classified for the full-session gate.
enum Reply {
    /// The peer answered with a `pong` carrying the ping id it echoes.
    Pong(i64),
    /// The peer reported a stale salt and the salt it expects instead.
    BadSalt(i64),
    /// Any other message, described for a divergence report.
    Other(String),
}

/// Classifies a decrypted peer body, looking through containers and RPC results.
fn classify(body: &[u8]) -> Reply {
    let mut decoder = Decoder::new(body, DecodeLimits::default());
    match Object::decode(&mut decoder) {
        Ok(object) => classify_object(&object),
        Err(error) => Reply::Other(format!("undecodable reply: {error}")),
    }
}

/// Classifies one decoded object, unwrapping containers and RPC results.
fn classify_object(object: &Object) -> Reply {
    match object {
        Object::Pong(pong) => Reply::Pong(pong.ping_id),
        Object::RpcResult(result) => match result.as_ref() {
            RpcResult::Rpc_result { result, .. } => classify_object(result),
        },
        Object::MessageContainer(container) => match container.as_ref() {
            MessageContainer::Msg_container { messages } => messages
                .iter()
                .map(|message| classify_object(&message.body))
                .find(|reply| !matches!(reply, Reply::Other(_)))
                .unwrap_or_else(|| Reply::Other("container carried no pong".to_owned())),
        },
        Object::BadMsgNotification(note) => match note.as_ref() {
            BadMsgNotification::Bad_server_salt {
                new_server_salt, ..
            } => Reply::BadSalt(*new_server_salt),
            other => Reply::Other(format!("bad-message notification {other:?}")),
        },
        other => Reply::Other(format!("unexpected constructor {:#010x}", other.type_id())),
    }
}

/// Encrypts one MTProto 2.0 message from the client and writes it to `stream`.
fn send_encrypted(
    stream: &mut CodecStream,
    auth_key: &AuthKey,
    session: &Session,
    message: MessageSeq,
    body: &[u8],
    random: &mut SystemRandom,
) {
    let header = MessageHeader {
        salt: session.salt(),
        session_id: session.session_id(),
        msg_id: message.msg_id,
        seq_no: message.seq_no,
    };
    let frame = encrypt(auth_key, &header, body, Side::Client, random).encode();
    stream
        .send(&frame)
        .expect("write encrypted message to the peer");
}

/// Reads one encrypted server message and returns its decrypted body.
fn recv_encrypted(stream: &mut CodecStream, auth_key: &AuthKey) -> Vec<u8> {
    let frame = stream.recv().expect("read encrypted message from the peer");
    let message = EncryptedMessage::decode(&frame).expect("decode the peer's encrypted frame");
    decrypt(auth_key, &message, Side::Server)
        .expect("decrypt the peer's message")
        .body
}

/// Sends an encrypted service `ping` and returns once the peer answers with a
/// matching `pong`; a stale-salt notification is adopted and the ping resent.
fn ping_round_trip(
    stream: &mut CodecStream,
    auth_key: &AuthKey,
    session: &mut Session,
    ping_id: i64,
    random: &mut SystemRandom,
) {
    let mut writer = Writer::new();
    Ping { ping_id }.encode(&mut writer);
    let body = writer.into_vec();

    let message = session.allocate(MessageType::FromClient, MessageClass::Service);
    send_encrypted(stream, auth_key, session, message, &body, random);

    let mut last = None;
    for _ in 0..8 {
        match classify(&recv_encrypted(stream, auth_key)) {
            Reply::Pong(answered) => {
                assert_eq!(
                    answered, ping_id,
                    "the peer answered ping {ping_id} with a pong for {answered}"
                );
                return;
            }
            Reply::BadSalt(salt) => {
                session.set_salt(salt);
                send_encrypted(stream, auth_key, session, message, &body, random);
            }
            Reply::Other(description) => last = Some(description),
        }
    }
    panic!(
        "the peer did not answer ping {ping_id} with a pong (last reply: {})",
        last.as_deref().unwrap_or("none")
    );
}

/// Runs `f` on a worker thread and fails fast if it does not finish in time.
///
/// A non-answering test server (for example, one that never prints its
/// advertised address) would otherwise block the reader forever and burn the
/// whole CI job's time budget; the watchdog turns that into a prompt failure.
fn run_with_timeout<F: FnOnce() + Send + 'static>(label: &str, timeout: Duration, f: F) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        f();
        let _ = tx.send(());
    });
    match rx.recv_timeout(timeout) {
        Ok(()) => {}
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("{label}: timed out after {timeout:?} (test server did not answer)")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("{label}: failed (worker thread panicked; see the panic above)")
        }
    }
}

#[test]
fn exchange_completes_against_official_testserver() {
    let Ok(binary) = std::env::var("RTD_PEER_TESTSERVER") else {
        eprintln!("RTD_PEER_TESTSERVER is unset; skipping the external-peer gate");
        return;
    };
    run_with_timeout(
        "external-peer gate (exchange)",
        Duration::from_secs(120),
        move || exchange_gate(&binary),
    );
}

/// The gate body, run under a watchdog so a stuck read cannot hang CI.
fn exchange_gate(binary: &str) {
    let peer = start_peer(binary);
    let public_key =
        rtd_crypto::parse_rsa_public_key(&peer.public_key_pem).expect("parse peer public key");

    let mut stream = CodecStream::connect(&peer.address, 2).expect("connect to peer");
    let config = ClientConfig {
        dc: 2,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public_key],
    };
    let clock = SystemClock;
    let mut random = SystemRandom;
    let result = match client_exchange(&mut stream, &config, &clock, &mut random) {
        Ok(result) => result,
        Err(error) => panic!(
            "key exchange with the official test server at {} failed: {error}",
            peer.address
        ),
    };

    assert!(!result.auth_key.value.is_zero());
    assert_ne!(result.session_id, 0);

    // Full encrypted session: seed a client session from the exchange and
    // require the peer's `pong` for an encrypted service `ping`. If the peer
    // uses a different session salt than the exchange-derived one it first
    // reports that salt, which `ping_round_trip` adopts before resending.
    let mut session = Session::with_id(Arc::new(SystemClock), result.session_id);
    session.set_salt(result.server_salt);
    let ping_id = i64::from_le_bytes(
        rtd_crypto::random_bytes(8)
            .try_into()
            .expect("eight random bytes"),
    );
    ping_round_trip(
        &mut stream,
        &result.auth_key,
        &mut session,
        ping_id,
        &mut random,
    );
}
