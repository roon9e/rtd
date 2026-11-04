//! External-peer gate: run the official engine's test server, connect the
//! client over obfuscated TCP, complete the authorization-key exchange and make
//! an RPC call.
//!
//! The official engine is GPL-2.0 and is never linked or vendored; this test
//! only *runs* its published binary as a peer. Set `RTD_PEER_TESTSERVER` to the
//! `mtproto-testserver` binary path to enable the gate; without it the test is
//! skipped so the workspace still builds and tests where the peer is absent.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use rtd_client::{Client, Error, InitParams, connect_tcp};
use rtd_exchange::ClientConfig;
use rtd_proto::CodecKind;
use rtd_util::SystemClock;

/// Runs `f` on a worker thread and fails fast if it does not finish in time.
///
/// A non-answering test server would otherwise block a read forever and burn the
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
fn client_invokes_against_official_testserver() {
    let Ok(binary) = std::env::var("RTD_PEER_TESTSERVER") else {
        eprintln!("RTD_PEER_TESTSERVER is unset; skipping the external-peer gate");
        return;
    };
    run_with_timeout(
        "external-peer gate (client)",
        Duration::from_secs(120),
        move || client_gate(&binary),
    );
}

/// The gate body, run under a watchdog so a stuck read cannot hang CI.
fn client_gate(binary: &str) {
    let peer = start_peer(binary);
    let public_key =
        rtd_crypto::parse_rsa_public_key(&peer.public_key_pem).expect("parse peer public key");

    // The official test server accepts obfuscated2 on TCP; the secret is empty.
    let transport = connect_tcp(&peer.address, 2, CodecKind::Intermediate, Some(&[]))
        .expect("connect to the peer");
    let clock: Arc<dyn rtd_util::Clock + Send + Sync> = Arc::new(SystemClock);
    let config = ClientConfig {
        dc: 2,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public_key],
    };
    let mut client =
        Client::connect(transport, &config, clock).expect("exchange completes with the peer");

    let result = client.initialize(InitParams {
        api_id: 2040,
        device_model: "rtd".to_owned(),
        system_version: "test".to_owned(),
        app_version: "0.0.0".to_owned(),
        system_lang_code: "en".to_owned(),
        lang_pack: String::new(),
        lang_code: "en".to_owned(),
    });
    match result {
        // A real reply (a `config`) proves the whole path.
        Ok(_) => {}
        // The server answering with an RPC error still proves the encrypted RPC
        // round trip; only a transport/timeout failure would be a regression.
        Err(Error::Rpc(_)) => {}
        Err(other) => panic!("unexpected client failure: {other}"),
    }
}
