//! The core-boundary trait and its in-process implementation.
//!
//! Owned by task group 10.1. Implemented in this file.
//!
//! The edge reaches durable state and method execution through the object-safe
//! [`Core`] trait: protocol negotiation, dispatch of an admitted call, the
//! durable authorization-key store, session-layer evidence and session-offline
//! reporting. [`LocalCore`] implements it in-process over a [`Router`]; the
//! gRPC adapter implements the same trait in the `rtd-edge` binary. The
//! request/result value types defined here are rtd-native and deliberately do
//! not reference the wire contract.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use rtd_bin::Writer;
use rtd_error::Error as RtdError;
use rtd_tlprofile::{
    CANONICAL_LAYER, Limits, Profile, SemanticId, admit_unprofiled_with_limits, admit_with_limits,
};

use crate::rpc::{self, HandlerRouter, Router, ServerConfig};

/// A boxed, sendable future used by the object-safe [`Core`] trait.
pub type CoreFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The coreexec protocol version implemented by this crate.
pub const CORE_PROTOCOL_VERSION: u32 = 1;

/// An authorization-key identifier (the low 64 bits of the key digest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthKeyId(pub [u8; 8]);

/// A session identifier at the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub i64);

/// A durable authorization-key record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthKeyRecord {
    /// The key identifier.
    pub id: AuthKeyId,
    /// The datacenter the key belongs to.
    pub dc: i32,
    /// The raw authorization-key bytes.
    pub key: Vec<u8>,
}

/// Durable session-layer evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerEvidence {
    /// The session the evidence belongs to.
    pub session: SessionId,
    /// The negotiated layer.
    pub layer: i32,
}

/// The core's protocol identity, negotiated at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreInfo {
    /// The coreexec protocol version.
    pub protocol_version: u32,
    /// The serving datacenter.
    pub dc: i32,
    /// Advertised capability names.
    pub capabilities: Vec<String>,
}

/// An admitted call handed to the core.
///
/// The edge admits the call locally and forwards the canonical request bytes
/// together with the metadata needed to bind and validate the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedCall {
    /// The session that admitted the call.
    pub session: SessionId,
    /// The stable semantic method identity.
    pub semantic: SemanticId,
    /// The negotiated profile layer, if any.
    pub layer: Option<i32>,
    /// The terminal wire identifier.
    pub wire_id: u32,
    /// The canonical request bytes.
    pub request: Vec<u8>,
}

/// The core's canonical result for an admitted call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallResult {
    /// The canonical result bytes, including the constructor identifier.
    pub result: Vec<u8>,
}

/// Returns the TL error surfaced when the core cannot serve a call.
#[must_use]
pub fn core_failure_error() -> RtdError {
    RtdError::new(500, "CORE_FAILURE")
}

/// The edge-facing core boundary.
///
/// Every method returns a boxed [`CoreFuture`] so the trait stays object-safe
/// without `async-trait`; a core failure is reported as a [`rtd_error::Error`]
/// TL error and the session stays alive.
pub trait Core: Send + Sync {
    /// Negotiates the core's protocol identity and capabilities.
    fn info(&self) -> CoreFuture<'_, Result<CoreInfo, RtdError>>;

    /// Executes an admitted call, returning canonical result bytes or a TL error.
    fn dispatch(&self, call: AdmittedCall) -> CoreFuture<'_, Result<CallResult, RtdError>>;

    /// Durably saves an authorization key before the exchange is confirmed.
    fn save_auth_key(&self, key: AuthKeyRecord) -> CoreFuture<'_, Result<(), RtdError>>;

    /// Fetches a durable authorization key, if any.
    fn get_auth_key(
        &self,
        id: AuthKeyId,
    ) -> CoreFuture<'_, Result<Option<AuthKeyRecord>, RtdError>>;

    /// Resolves the durable layer evidence for a session, if any.
    fn resolve_layer_evidence(
        &self,
        session: SessionId,
    ) -> CoreFuture<'_, Result<Option<i32>, RtdError>>;

    /// Advances the durable layer evidence for a session (only upward).
    fn advance_layer_evidence(
        &self,
        session: SessionId,
        layer: i32,
    ) -> CoreFuture<'_, Result<(), RtdError>>;

    /// Publishes durable layer evidence for a session.
    fn publish_layer_evidence(
        &self,
        evidence: LayerEvidence,
    ) -> CoreFuture<'_, Result<(), RtdError>>;

    /// Reports that a session went offline.
    fn report_session_offline(&self, session: SessionId) -> CoreFuture<'_, Result<(), RtdError>>;
}

/// An in-process [`Core`] over a [`Router`] and an in-memory durable store.
pub struct LocalCore<R: Router = HandlerRouter> {
    router: R,
    dc: i32,
    keys: Mutex<HashMap<AuthKeyId, AuthKeyRecord>>,
    evidence: Mutex<HashMap<SessionId, i32>>,
}

impl<R: Router> LocalCore<R> {
    /// Creates an in-process core over `router` serving datacenter `dc`.
    #[must_use]
    pub fn new(router: R, dc: i32) -> Self {
        Self {
            router,
            dc,
            keys: Mutex::new(HashMap::new()),
            evidence: Mutex::new(HashMap::new()),
        }
    }
}

impl LocalCore<HandlerRouter> {
    /// Creates an in-process core with the built-in methods wired to `config`.
    #[must_use]
    pub fn with_builtins(config: &ServerConfig) -> Self {
        Self::new(HandlerRouter::with_builtins(config), config.dc)
    }
}

impl<R: Router> Core for LocalCore<R> {
    fn info(&self) -> CoreFuture<'_, Result<CoreInfo, RtdError>> {
        let dc = self.dc;
        Box::pin(async move {
            Ok(CoreInfo {
                protocol_version: CORE_PROTOCOL_VERSION,
                dc,
                capabilities: Vec::new(),
            })
        })
    }

    fn dispatch(&self, call: AdmittedCall) -> CoreFuture<'_, Result<CallResult, RtdError>> {
        Box::pin(async move {
            let limits = Limits::default();
            let admission = match call.layer {
                Some(layer) => admit_with_limits(Profile::new(layer), &call.request, limits),
                None => admit_unprofiled_with_limits(&call.request, limits),
            }
            .map_err(|error| rpc::map_admit_error(&error))?;
            if admission.semantic != call.semantic || admission.wire_id != call.wire_id {
                return Err(rpc::call_identity_error());
            }
            let result = rpc::dispatch(&self.router, &admission)?;
            let profile = admission
                .effective_profile
                .unwrap_or(Profile::new(CANONICAL_LAYER));
            let mut writer = Writer::new();
            result
                .encode(profile, &mut writer)
                .map_err(|error| rpc::map_projection_error(&error))?;
            Ok(CallResult {
                result: writer.into_vec(),
            })
        })
    }

    fn save_auth_key(&self, key: AuthKeyRecord) -> CoreFuture<'_, Result<(), RtdError>> {
        Box::pin(async move {
            self.keys
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(key.id, key);
            Ok(())
        })
    }

    fn get_auth_key(
        &self,
        id: AuthKeyId,
    ) -> CoreFuture<'_, Result<Option<AuthKeyRecord>, RtdError>> {
        Box::pin(async move {
            let key = self
                .keys
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&id)
                .cloned();
            Ok(key)
        })
    }

    fn resolve_layer_evidence(
        &self,
        session: SessionId,
    ) -> CoreFuture<'_, Result<Option<i32>, RtdError>> {
        Box::pin(async move {
            let layer = self
                .evidence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&session)
                .copied();
            Ok(layer)
        })
    }

    fn advance_layer_evidence(
        &self,
        session: SessionId,
        layer: i32,
    ) -> CoreFuture<'_, Result<(), RtdError>> {
        Box::pin(async move {
            let mut evidence = self
                .evidence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let entry = evidence.entry(session).or_insert(layer);
            if layer > *entry {
                *entry = layer;
            }
            Ok(())
        })
    }

    fn publish_layer_evidence(
        &self,
        evidence: LayerEvidence,
    ) -> CoreFuture<'_, Result<(), RtdError>> {
        Box::pin(async move {
            self.evidence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(evidence.session, evidence.layer);
            Ok(())
        })
    }

    fn report_session_offline(&self, session: SessionId) -> CoreFuture<'_, Result<(), RtdError>> {
        Box::pin(async move {
            self.evidence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&session);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::RoutedResult;
    use rtd_tg::{HelpGetConfig, Method, Object};
    use rtd_tlprofile::semantic::METHOD_HELP_GET_CONFIG;
    use rtd_tlprofile::{Handler, decode_object};

    fn config() -> ServerConfig {
        ServerConfig::new(2, vec![crate::rpc::DataCenter::new(2, "127.0.0.1", 443)])
    }

    fn config_call() -> AdmittedCall {
        let mut writer = Writer::new();
        Method::HelpGetConfig(Box::new(HelpGetConfig)).encode(&mut writer);
        AdmittedCall {
            session: SessionId(7),
            semantic: METHOD_HELP_GET_CONFIG,
            layer: Some(232),
            wire_id: HelpGetConfig::TYPE_ID,
            request: writer.into_vec(),
        }
    }

    #[tokio::test]
    async fn in_process_dispatch() {
        let core = LocalCore::with_builtins(&config());
        let result = core.dispatch(config_call()).await.expect("dispatch");
        let decoded = decode_object(Profile::new(232), &result.result, Limits::default())
            .expect("decode result");
        let Object::Config(returned) = decoded else {
            panic!("expected a Config result");
        };
        assert_eq!(returned.this_dc, 2);
        assert_eq!(returned.dc_options.len(), 1);
    }

    #[tokio::test]
    async fn core_is_object_safe() {
        let core: Box<dyn Core> = Box::new(LocalCore::with_builtins(&config()));
        let info = core.info().await.expect("info");
        assert_eq!(info.protocol_version, CORE_PROTOCOL_VERSION);
        assert_eq!(info.dc, 2);
    }

    #[tokio::test]
    async fn auth_key_round_trip() {
        let core = LocalCore::new(HandlerRouter::new(), 2);
        let id = AuthKeyId([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(core.get_auth_key(id).await.expect("get"), None);
        let record = AuthKeyRecord {
            id,
            dc: 2,
            key: vec![0xaa; 256],
        };
        core.save_auth_key(record.clone()).await.expect("save");
        assert_eq!(core.get_auth_key(id).await.expect("get"), Some(record));
    }

    #[tokio::test]
    async fn layer_evidence_round_trip() {
        let core = LocalCore::new(HandlerRouter::new(), 2);
        let session = SessionId(3);
        assert_eq!(
            core.resolve_layer_evidence(session).await.expect("resolve"),
            None
        );
        core.advance_layer_evidence(session, 231)
            .await
            .expect("advance");
        core.advance_layer_evidence(session, 230)
            .await
            .expect("advance down ignored");
        assert_eq!(
            core.resolve_layer_evidence(session).await.expect("resolve"),
            Some(231)
        );
        core.publish_layer_evidence(LayerEvidence {
            session,
            layer: 232,
        })
        .await
        .expect("publish");
        assert_eq!(
            core.resolve_layer_evidence(session).await.expect("resolve"),
            Some(232)
        );
        core.report_session_offline(session).await.expect("offline");
        assert_eq!(
            core.resolve_layer_evidence(session).await.expect("resolve"),
            None
        );
    }

    struct FailingRouter;

    impl Router for FailingRouter {
        fn register(&mut self, _semantic: SemanticId, _handler: Box<dyn Handler + Send + Sync>) {}

        fn route(&self, _admission: &rtd_tlprofile::Admission) -> Result<RoutedResult, RtdError> {
            Err(core_failure_error())
        }
    }

    #[tokio::test]
    async fn core_failure_mapped() {
        let core = LocalCore::new(FailingRouter, 2);
        let error = core.dispatch(config_call()).await.expect_err("core fails");
        assert_eq!(error.code(), 500);
        assert_eq!(error.message(), "CORE_FAILURE");
        // The session (and the core object) stays usable after a failure.
        assert!(core.info().await.is_ok());
    }

    #[tokio::test]
    async fn malformed_request_mapped() {
        let core = LocalCore::with_builtins(&config());
        let mut call = config_call();
        call.request = vec![0xff, 0xff, 0xff, 0xff];
        let error = core.dispatch(call).await.expect_err("decode fails");
        assert_eq!(error.code(), rpc::CODE_BAD_REQUEST);
    }

    #[tokio::test]
    async fn boundary_metadata_mismatch_rejected() {
        let core = LocalCore::with_builtins(&config());
        let mut call = config_call();
        call.wire_id = 0;
        let error = core.dispatch(call).await.expect_err("identity mismatch");
        assert_eq!(error.message(), rpc::ERR_CALL_IDENTITY);
    }
}
