//! Smoke test: bind the edge on an ephemeral port and shut it down cleanly.

use std::sync::Arc;
use std::time::Duration;

use rsa::RsaPrivateKey;
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use rtd_edge::{EdgeConfig, service};
use rtd_server::actor::{BudgetLimits, Exchange, Server, ServerOptions};
use rtd_server::core::LocalCore;
use rtd_server::ingress::IngressConfig;
use rtd_server::rpc::{DataCenter, ServerConfig};
use rtd_tlprofile::{CANONICAL_LAYER, Profile};
use rtd_util::{Clock, SystemClock};

#[tokio::test]
async fn binds_an_ephemeral_port_and_shuts_down() {
    let core = Arc::new(LocalCore::with_builtins(&ServerConfig::new(
        2,
        vec![DataCenter::new(2, "127.0.0.1", 443)],
    )));
    let exchange: Arc<dyn Exchange> = Arc::new(
        |_: &mut dyn rtd_exchange::Stream,
         _: &dyn Clock,
         _: &mut dyn rtd_crypto::RandomSource|
         -> rtd_exchange::Result<rtd_exchange::ServerExchangeResult> {
            Err(rtd_exchange::Error::Decode(
                "unused in the smoke test".to_owned(),
            ))
        },
    );
    let options = ServerOptions {
        dc: 2,
        clock: Arc::new(SystemClock),
        exchange,
        layer: Profile::new(CANONICAL_LAYER),
        idle_timeout: Duration::from_secs(300),
        budget_limits: BudgetLimits::default(),
    };
    let server = Server::bind("127.0.0.1:0", IngressConfig::default(), core, options)
        .await
        .expect("bind the edge");
    let addr = server.local_addr().expect("local address");
    assert_ne!(addr.port(), 0, "an ephemeral port was assigned");

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(async move {
        service::run_until(server, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    shutdown_tx.send(()).expect("signal shutdown");
    running
        .await
        .expect("join the serving task")
        .expect("clean shutdown");
}

#[tokio::test]
async fn binds_from_configuration_with_an_in_process_core() {
    // Roughly the documented in-process run: a PEM key, no core endpoint, an
    // ephemeral port. A small key is enough because `bind` never runs the
    // exchange.
    let key = RsaPrivateKey::new(&mut rand::thread_rng(), 512).expect("generate key");
    let pem = key.to_pkcs8_pem(LineEnding::LF).expect("encode PEM");
    let path = std::env::temp_dir().join(format!("rtd-edge-smoke-{}.pem", std::process::id()));
    std::fs::write(&path, pem.as_bytes()).expect("write PEM");

    let config = EdgeConfig {
        bind: "127.0.0.1:0".to_owned(),
        dc: 2,
        layer: rtd_tlprofile::CANONICAL_LAYER,
        rsa_key_path: path.clone(),
        secret: None,
        websocket: false,
        core_endpoint: None,
        core_token: None,
    };
    let server = service::bind(config).await.expect("bind the edge");
    assert_ne!(server.local_addr().expect("local address").port(), 0);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(service::run_until(server, async move {
        let _ = shutdown_rx.await;
    }));
    shutdown_tx.send(()).expect("signal shutdown");
    running
        .await
        .expect("join the serving task")
        .expect("clean shutdown");

    let _ = std::fs::remove_file(&path);
}
