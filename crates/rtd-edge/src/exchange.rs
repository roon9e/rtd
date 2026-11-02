//! The server-side key-exchange strategy supplied to the connection actor.
//!
//! The actor is transport- and key-agnostic: it delegates the authorization-key
//! exchange to an [`Exchange`] value. This module implements that trait with
//! [`rtd_exchange::server_exchange`] over the configured RSA private key, so the
//! actor never links an RSA key type itself.

use std::sync::Arc;

use rsa::RsaPrivateKey;
use rtd_exchange::{ServerConfig, ServerExchangeResult, TestServerRng};
use rtd_server::actor::Exchange;
use rtd_util::Clock;

/// A server-side [`Exchange`] over a configured RSA private key.
///
/// Each call runs [`rtd_exchange::server_exchange`] with the edge's datacenter
/// and key. The Diffie-Hellman parameters currently come from
/// [`TestServerRng`], the fixed development generator; selecting a production
/// parameter generator is a follow-up.
#[derive(Debug, Clone)]
pub struct ServerExchange {
    key: Arc<RsaPrivateKey>,
    dc: i32,
}

impl ServerExchange {
    /// Creates an exchange strategy for datacenter `dc` using `key`.
    #[must_use]
    pub fn new(key: Arc<RsaPrivateKey>, dc: i32) -> Self {
        Self { key, dc }
    }
}

impl Exchange for ServerExchange {
    fn exchange(
        &self,
        stream: &mut dyn rtd_exchange::Stream,
        clock: &dyn Clock,
        random: &mut dyn rtd_crypto::RandomSource,
    ) -> rtd_exchange::Result<ServerExchangeResult> {
        let key: &RsaPrivateKey = &self.key;
        // Match the reference edge's `strict_dc_check: false`: accept every DC
        // label, since one physical edge answers for all of them and clients try
        // several before one sticks.
        let config = ServerConfig {
            dc: self.dc,
            key,
            strict_dc: false,
        };
        let mut rng = TestServerRng;
        let mut stream = StreamRef(stream);
        let mut random = RandomRef(random);
        rtd_exchange::server_exchange(&mut stream, &config, &mut rng, clock, &mut random)
    }
}

/// A sized [`rtd_exchange::Stream`] adapter over the actor's trait object.
///
/// [`rtd_exchange::server_exchange`] takes `&mut impl Stream`, which requires a
/// sized stream; the actor hands the exchange a `&mut dyn Stream`, so this thin
/// newtype restores a sized wrapper that forwards both operations.
struct StreamRef<'a>(&'a mut dyn rtd_exchange::Stream);

impl rtd_exchange::Stream for StreamRef<'_> {
    fn send(&mut self, frame: &[u8]) -> rtd_exchange::Result<()> {
        self.0.send(frame)
    }

    fn recv(&mut self) -> rtd_exchange::Result<Vec<u8>> {
        self.0.recv()
    }
}

/// A sized [`rtd_crypto::RandomSource`] adapter over the actor's trait object.
///
/// Mirrors [`StreamRef`]: [`rtd_exchange::server_exchange`] takes
/// `&mut impl RandomSource`, so the `&mut dyn RandomSource` handed to the
/// exchange is wrapped to satisfy the implicit `Sized` bound.
struct RandomRef<'a>(&'a mut dyn rtd_crypto::RandomSource);

impl rtd_crypto::RandomSource for RandomRef<'_> {
    fn fill(&mut self, dest: &mut [u8]) {
        self.0.fill(dest);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rsa::RsaPrivateKey;
    use rtd_server::actor::Exchange;
    use rtd_util::SystemClock;

    use super::ServerExchange;

    /// A stream whose first read fails, terminating the exchange early.
    struct FailingStream;

    impl rtd_exchange::Stream for FailingStream {
        fn send(&mut self, _frame: &[u8]) -> rtd_exchange::Result<()> {
            Err(rtd_exchange::Error::Stream("send failed".to_owned()))
        }

        fn recv(&mut self) -> rtd_exchange::Result<Vec<u8>> {
            Err(rtd_exchange::Error::Stream("closed".to_owned()))
        }
    }

    #[test]
    fn propagates_a_stream_failure() {
        // A small key is enough: the failing stream ends the exchange before the
        // key is used, and the adapter's key handling is what this exercises.
        let key =
            Arc::new(RsaPrivateKey::new(&mut rand::thread_rng(), 512).expect("generate test key"));
        let exchange = ServerExchange::new(key, 2);
        let mut stream = FailingStream;
        let mut random = rtd_crypto::SystemRandom;
        let error = exchange
            .exchange(&mut stream, &SystemClock, &mut random)
            .expect_err("the stream fails");
        assert!(matches!(error, rtd_exchange::Error::Stream(_)));
    }
}
