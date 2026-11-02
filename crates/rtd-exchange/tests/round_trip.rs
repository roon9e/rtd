//! Drives the client and server exchange flows against each other over an
//! in-memory duplex.

use std::sync::mpsc::{Receiver, Sender, channel};

use rand::SeedableRng;
use rand::rngs::StdRng;
use rsa::RsaPrivateKey;

use rtd_crypto::SystemRandom;
use rtd_exchange::{
    ClientConfig, Error, Result, ServerConfig, Stream, TestServerRng, client_exchange,
    server_exchange,
};
use rtd_util::SystemClock;

/// A [`Stream`] backed by two channels.
struct ChannelStream {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
}

impl Stream for ChannelStream {
    fn send(&mut self, frame: &[u8]) -> Result<()> {
        self.tx
            .send(frame.to_vec())
            .map_err(|error| Error::Stream(error.to_string()))
    }

    fn recv(&mut self) -> Result<Vec<u8>> {
        self.rx
            .recv()
            .map_err(|error| Error::Stream(error.to_string()))
    }
}

#[test]
fn client_and_server_derive_the_same_key() {
    let mut rng = StdRng::seed_from_u64(7);
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
    let public = private.to_public_key();

    let (client_tx, server_rx) = channel();
    let (server_tx, client_rx) = channel();

    let server_thread = std::thread::spawn(move || {
        let clock = SystemClock;
        let mut stream = ChannelStream {
            tx: server_tx,
            rx: server_rx,
        };
        let config = ServerConfig {
            dc: 2,
            key: &private,
            strict_dc: true,
        };
        let mut parameters = TestServerRng;
        let mut random = SystemRandom;
        server_exchange(&mut stream, &config, &mut parameters, &clock, &mut random)
    });

    let clock = SystemClock;
    let mut stream = ChannelStream {
        tx: client_tx,
        rx: client_rx,
    };
    let config = ClientConfig {
        dc: 2,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public],
    };
    let mut random = SystemRandom;
    let client = client_exchange(&mut stream, &config, &clock, &mut random).expect("client flow");
    let server = server_thread
        .join()
        .expect("server thread")
        .expect("server flow");

    assert_eq!(
        client.auth_key.value.as_bytes(),
        server.auth_key.value.as_bytes()
    );
    assert_eq!(client.auth_key.id, server.auth_key.id);
    assert_eq!(client.server_salt, server.server_salt);
    assert!(!client.auth_key.value.is_zero());
}

#[test]
fn temporary_key_mode_round_trips() {
    let mut rng = StdRng::seed_from_u64(11);
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
    let public = private.to_public_key();

    let (client_tx, server_rx) = channel();
    let (server_tx, client_rx) = channel();

    let server_thread = std::thread::spawn(move || {
        let clock = SystemClock;
        let mut stream = ChannelStream {
            tx: server_tx,
            rx: server_rx,
        };
        let config = ServerConfig {
            dc: 4,
            key: &private,
            strict_dc: true,
        };
        let mut parameters = TestServerRng;
        let mut random = SystemRandom;
        server_exchange(&mut stream, &config, &mut parameters, &clock, &mut random)
    });

    let clock = SystemClock;
    let mut stream = ChannelStream {
        tx: client_tx,
        rx: client_rx,
    };
    let config = ClientConfig {
        dc: 4,
        temporary: true,
        expires_in: 86_400,
        public_keys: vec![public],
    };
    let mut random = SystemRandom;
    let client = client_exchange(&mut stream, &config, &clock, &mut random).expect("client flow");
    let server = server_thread
        .join()
        .expect("server thread")
        .expect("server flow");

    assert_eq!(
        client.auth_key.value.as_bytes(),
        server.auth_key.value.as_bytes()
    );
    assert_eq!(client.server_salt, server.server_salt);
}

#[test]
fn wrong_datacenter_is_rejected() {
    let mut rng = StdRng::seed_from_u64(3);
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
    let public = private.to_public_key();

    let (client_tx, server_rx) = channel();
    let (server_tx, client_rx) = channel();

    let server_thread = std::thread::spawn(move || {
        let clock = SystemClock;
        let mut stream = ChannelStream {
            tx: server_tx,
            rx: server_rx,
        };
        let config = ServerConfig {
            dc: 2,
            key: &private,
            strict_dc: true,
        };
        let mut parameters = TestServerRng;
        let mut random = SystemRandom;
        server_exchange(&mut stream, &config, &mut parameters, &clock, &mut random)
    });

    let clock = SystemClock;
    let mut stream = ChannelStream {
        tx: client_tx,
        rx: client_rx,
    };
    let config = ClientConfig {
        dc: 5,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public],
    };
    let mut random = SystemRandom;
    let client = client_exchange(&mut stream, &config, &clock, &mut random);
    let server = server_thread.join().expect("server thread");

    assert!(client.is_err(), "client should not complete");
    assert!(
        matches!(server, Err(Error::WrongDc { want: 2, got: 5 })),
        "server should reject the datacenter, got {server:?}"
    );
}

#[test]
fn non_strict_accepts_a_foreign_datacenter() {
    let mut rng = StdRng::seed_from_u64(4);
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
    let public = private.to_public_key();

    let (client_tx, server_rx) = channel();
    let (server_tx, client_rx) = channel();

    let server_thread = std::thread::spawn(move || {
        let clock = SystemClock;
        let mut stream = ChannelStream {
            tx: server_tx,
            rx: server_rx,
        };
        // The reference edge runs non-strict: it answers for every DC label, so a
        // client that tries DC 5 against this DC-2 backend still completes.
        let config = ServerConfig {
            dc: 2,
            key: &private,
            strict_dc: false,
        };
        let mut parameters = TestServerRng;
        let mut random = SystemRandom;
        server_exchange(&mut stream, &config, &mut parameters, &clock, &mut random)
    });

    let clock = SystemClock;
    let mut stream = ChannelStream {
        tx: client_tx,
        rx: client_rx,
    };
    let config = ClientConfig {
        dc: 5,
        temporary: false,
        expires_in: 0,
        public_keys: vec![public],
    };
    let mut random = SystemRandom;
    let client =
        client_exchange(&mut stream, &config, &clock, &mut random).expect("client completes");
    let server = server_thread
        .join()
        .expect("server thread")
        .expect("server completes");

    assert_eq!(
        client.auth_key.value.as_bytes(),
        server.auth_key.value.as_bytes()
    );
    assert_eq!(client.server_salt, server.server_salt);
}
