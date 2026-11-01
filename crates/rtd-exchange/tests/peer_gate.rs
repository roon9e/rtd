//! External-peer gate: run the official engine's test server and complete the
//! authorization-key exchange against it over a real socket.
//!
//! The official engine is GPL-2.0 and is never linked or vendored; this test
//! only *runs* its published binary as a peer. Set `RTD_PEER_TESTSERVER` to the
//! `mtproto-testserver` binary path to enable the gate; without it the test is
//! skipped so the workspace still builds and tests where the peer is absent.

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use rtd_crypto::SystemRandom;
use rtd_exchange::{ClientConfig, Error, Result, Stream, client_exchange};
use rtd_proto::{Codec, Intermediate};
use rtd_transport::Obfuscated2;
use rtd_util::SystemClock;

/// A [`Stream`] over an obfuscated2 TCP connection using intermediate framing.
///
/// The official test server accepts only obfuscated2 on TCP; the framing is
/// carried by the obfuscation protocol tag, so no plain transport header is
/// written.
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

#[test]
fn exchange_completes_against_official_testserver() {
    let Ok(binary) = std::env::var("RTD_PEER_TESTSERVER") else {
        eprintln!("RTD_PEER_TESTSERVER is unset; skipping the external-peer gate");
        return;
    };

    let peer = start_peer(&binary);
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
}
