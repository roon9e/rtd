//! Assembling and running the edge from configuration.
//!
//! [`bind`] turns an [`EdgeConfig`] into a bound [`Server`]: it loads the RSA
//! key, selects the core (the remote [`GrpcCore`] when a core endpoint is
//! configured, otherwise the in-process [`LocalCore`]), and applies the ingress
//! transports. [`run`] then serves until the process receives a shutdown
//! signal; [`run_until`] is the same loop with an injected shutdown future.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use rtd_server::actor::{BudgetLimits, Exchange, Server, ServerOptions};
use rtd_server::core::{Core, LocalCore};
use rtd_server::ingress::IngressConfig;
use rtd_server::rpc::{DataCenter, ServerConfig};
use rtd_tlprofile::{CANONICAL_LAYER, Profile};
use rtd_util::SystemClock;
use tonic::transport::Endpoint;

use crate::config::EdgeConfig;
use crate::error::EdgeError;
use crate::exchange::ServerExchange;
use crate::grpc::GrpcCore;

/// The idle timeout after which a transient session record is evicted.
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// The port advertised for the serving datacenter's built-in `help.getConfig`.
const ADVERTISED_PORT: i32 = 443;

/// Builds and binds the edge described by `config`.
///
/// # Errors
///
/// Returns an error when the RSA key cannot be loaded, the core endpoint cannot
/// be dialed, or the listener cannot be bound.
pub async fn bind(config: EdgeConfig) -> Result<Server, EdgeError> {
    let key = config.load_rsa_key()?;
    let core = build_core(&config).await?;
    let exchange: Arc<dyn Exchange> = Arc::new(ServerExchange::new(key, config.dc));
    let options = ServerOptions {
        dc: config.dc,
        clock: Arc::new(SystemClock),
        exchange,
        layer: Profile::new(CANONICAL_LAYER),
        idle_timeout: IDLE_TIMEOUT,
        budget_limits: BudgetLimits::default(),
    };
    let ingress = ingress_config(&config);
    Server::bind(config.bind, ingress, core, options)
        .await
        .map_err(EdgeError::Ingress)
}

/// Builds the edge and serves until the process receives a shutdown signal.
///
/// # Errors
///
/// Returns an error when the edge cannot be built or the listener fails.
pub async fn run(config: EdgeConfig) -> Result<(), EdgeError> {
    let server = bind(config).await?;
    run_until(server, shutdown_signal()).await
}

/// Serves `server` until `shutdown` resolves.
///
/// # Errors
///
/// Returns [`EdgeError::Ingress`] when accepting a connection fails.
pub async fn run_until(
    server: Server,
    shutdown: impl Future<Output = ()>,
) -> Result<(), EdgeError> {
    tokio::pin!(shutdown);
    tokio::select! {
        result = server.serve() => result.map_err(EdgeError::Ingress),
        () = &mut shutdown => Ok(()),
    }
}

/// Resolves when the process receives SIGINT (Ctrl-C).
pub async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("rtd-edge: could not install the shutdown handler: {error}");
    }
}

/// Selects the remote or in-process core.
async fn build_core(config: &EdgeConfig) -> Result<Arc<dyn Core>, EdgeError> {
    match &config.core_endpoint {
        Some(endpoint) => {
            // Connect lazily: the edge must start and listen even while the core
            // is still coming up, and reconnect once it is reachable, rather
            // than exiting and restart-looping.
            let endpoint = Endpoint::from_shared(endpoint.clone()).map_err(EdgeError::Core)?;
            let channel = endpoint.connect_lazy();
            let mut core = GrpcCore::new(channel);
            if let Some(token) = &config.core_token {
                core = core.with_token(token.clone());
            }
            Ok(Arc::new(core))
        }
        None => Ok(Arc::new(LocalCore::with_builtins(&server_config(
            config.dc,
        )))),
    }
}

/// Builds the ingress configuration for `config`.
fn ingress_config(config: &EdgeConfig) -> IngressConfig {
    let ingress = IngressConfig::new().with_websocket(config.websocket);
    match &config.secret {
        Some(secret) => ingress.with_secret(secret.clone()),
        None => ingress,
    }
}

/// Builds the in-process core's built-in configuration for datacenter `dc`.
fn server_config(dc: i32) -> ServerConfig {
    ServerConfig::new(dc, vec![DataCenter::new(dc, "127.0.0.1", ADVERTISED_PORT)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingress_config_carries_secret_and_websocket() {
        let config = EdgeConfig {
            bind: "127.0.0.1:0".to_owned(),
            dc: 2,
            rsa_key_path: "key.pem".into(),
            secret: Some(rtd_transport::Secret::parse("00112233445566778899aabbccddeeff").unwrap()),
            websocket: true,
            core_endpoint: None,
            core_token: None,
        };
        let ingress = ingress_config(&config);
        // The secret and WebSocket settings are only observable through their
        // effects on detection, so assert the configuration round-trips.
        assert!(ingress.secret.is_some());
        assert!(ingress.websocket);
    }

    #[test]
    fn advertised_datacenter_matches_the_serving_dc() {
        let config = server_config(5);
        assert_eq!(config.dc, 5);
        assert_eq!(config.datacenters.len(), 1);
        assert_eq!(config.datacenters[0].id, 5);
    }

    #[test]
    fn missing_key_is_a_usage_independent_error() {
        let config = EdgeConfig {
            bind: "127.0.0.1:0".to_owned(),
            dc: 2,
            rsa_key_path: "does-not-exist.pem".into(),
            secret: None,
            websocket: false,
            core_endpoint: None,
            core_token: None,
        };
        assert!(matches!(
            config.load_rsa_key().expect_err("missing file"),
            EdgeError::KeyFile { .. }
        ));
    }
}
