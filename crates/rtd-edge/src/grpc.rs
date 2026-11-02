//! The tonic gRPC adapter to a core's `CoreExecService`.
//!
//! This module implements [`rtd_server::core::Core`] by dialing the core's
//! `telesrv.coreexec.v1.CoreExecService` over tonic. It is the client half of
//! the boundary that [`rtd_edge_proto`] pins: each [`rtd_server::core::Core`]
//! value is encoded as one of the committed prost messages, sent as a unary
//! request at the matching [`rtd_edge_proto::methods`] path, and the answer is
//! decoded back into an rtd-native value. No `tonic-build` runs here; the
//! method-path constants and the committed prost types are the contract.
//!
//! A transport or control-plane failure is reported as a TL error (the
//! [`rtd_server::core::core_failure_error`] for a transport failure, the
//! `RPCError` the core returned for an application failure), so the caller
//! surfaces it to the client and keeps the session alive.

use rtd_edge_proto::methods;
use rtd_edge_proto::{
    AdmissionMode, AdvanceSessionLayerResponse, AuthKeyRecord as ProtoAuthKeyRecord,
    AuthKeyRequest, CoreExecInfoRequest, CoreExecInfoResponse, DispatchRequest, DispatchResponse,
    ErrorResponse, GetAuthKeyResponse, ProfileMark, PublishLayerEvidenceRequest,
    ResolveSessionLayerResponse, SessionOfflineBatchRequest, SessionOfflineEvent, WireProof,
};
use rtd_error::Error as RtdError;
use rtd_server::core::{
    AdmittedCall, AuthKeyId, AuthKeyRecord, CallResult, Core, CoreFuture, CoreInfo, LayerEvidence,
    SessionId, core_failure_error,
};
use tonic::Request;
use tonic::client::Grpc;
use tonic::codec::ProstCodec;
use tonic::codegen::http::uri::PathAndQuery;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint};

/// The TL error code reported for a control-plane failure at the boundary.
const CORE_FAILURE_CODE: i32 = 500;

/// The gRPC metadata key carrying the core-exec bearer credential.
const AUTHORIZATION_METADATA: &str = "authorization";

/// A [`Core`] implementation that dials a remote core's `CoreExecService`.
///
/// The adapter holds a connected [`Channel`]; each call clones it and performs
/// one unary RPC. Because the edge-facing [`Core`] value types are deliberately
/// narrower than the wire contract, the adapter carries only what the trait
/// exposes: it drops the edge-local datacenter on an [`AuthKeyRecord`] and
/// sends the unprofiled/absent session identifiers as their protobuf defaults.
#[derive(Debug, Clone)]
pub struct GrpcCore {
    channel: Channel,
    token: Option<String>,
}

impl GrpcCore {
    /// Creates an adapter over an already-connected channel.
    #[must_use]
    pub fn new(channel: Channel) -> Self {
        Self {
            channel,
            token: None,
        }
    }

    /// Sets the bearer token presented to the core on every call.
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Dials `endpoint` and returns a connected adapter.
    ///
    /// # Errors
    ///
    /// Returns the transport error when the endpoint cannot be reached.
    pub async fn connect(endpoint: Endpoint) -> Result<Self, tonic::transport::Error> {
        Ok(Self::new(endpoint.connect().await?))
    }

    /// Deletes a durable authorization key from the core.
    ///
    /// The edge-facing [`Core`] trait has no delete operation, so this exposes
    /// the `CoreExecService.DeleteAuthKey` RPC for callers that need it
    /// directly.
    ///
    /// # Errors
    ///
    /// Returns a [`rtd_error::Error`] when the core reports a failure.
    pub async fn delete_auth_key(&self, id: AuthKeyId) -> Result<(), RtdError> {
        let request = auth_key_request(id.0.to_vec(), 0, 0);
        let response: ErrorResponse = self.unary(request, methods::DELETE_AUTH_KEY).await?;
        control_result(response)
    }

    /// Sends one unary message on `path` and returns the decoded answer.
    async fn unary<M1, M2>(&self, message: M1, path: &'static str) -> Result<M2, RtdError>
    where
        M1: prost::Message + Send + Sync + 'static,
        M2: prost::Message + Default + Send + Sync + 'static,
    {
        let mut client = Grpc::new(self.channel.clone());
        client.ready().await.map_err(|_| core_failure_error())?;
        let codec = ProstCodec::<M1, M2>::default();
        let mut request = Request::new(message);
        self.authorize(&mut request)?;
        let response = client
            .unary(request, PathAndQuery::from_static(path), codec)
            .await
            .map_err(|_| core_failure_error())?;
        Ok(response.into_inner())
    }

    /// Inserts the configured bearer credential into `request`'s metadata.
    ///
    /// # Errors
    ///
    /// Returns a control-plane failure when the configured token is not valid
    /// ASCII metadata.
    fn authorize<M>(&self, request: &mut Request<M>) -> Result<(), RtdError> {
        apply_bearer(request, self.token.as_deref())
    }
}

/// Inserts `authorization: Bearer <token>` into `request` when `token` is set.
///
/// # Errors
///
/// Returns a control-plane failure when `token` is not valid ASCII metadata.
fn apply_bearer<M>(request: &mut Request<M>, token: Option<&str>) -> Result<(), RtdError> {
    let Some(token) = token else {
        return Ok(());
    };
    let value = MetadataValue::try_from(format!("Bearer {token}"))
        .map_err(|_| RtdError::new(CORE_FAILURE_CODE, "core-exec token is not valid metadata"))?;
    request.metadata_mut().insert(AUTHORIZATION_METADATA, value);
    Ok(())
}

impl Core for GrpcCore {
    fn info(&self) -> CoreFuture<'_, Result<CoreInfo, RtdError>> {
        Box::pin(async move {
            let request = CoreExecInfoRequest {
                protocol_version: rtd_edge_proto::PROTOCOL_VERSION,
                min_supported_protocol_version: rtd_edge_proto::MIN_SUPPORTED_PROTOCOL_VERSION,
                capabilities: rtd_edge_proto::CAPABILITIES
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
            };
            let response: CoreExecInfoResponse = self.unary(request, methods::GET_INFO).await?;
            if !response.error.is_empty() {
                return Err(RtdError::new(CORE_FAILURE_CODE, response.error));
            }
            rtd_edge_proto::verify_core_info(&response)
                .map_err(|error| RtdError::new(CORE_FAILURE_CODE, error.message))?;
            Ok(CoreInfo {
                protocol_version: response.protocol_version,
                dc: 0,
                capabilities: response.capabilities,
            })
        })
    }

    fn dispatch(&self, call: AdmittedCall) -> CoreFuture<'_, Result<CallResult, RtdError>> {
        Box::pin(async move {
            // The connection actor admitted with default inheritance, so the
            // core must re-admit in the matching default mode.
            let mode = if call.layer.is_some() {
                AdmissionMode::Default
            } else {
                AdmissionMode::Unprofiled
            };
            let profile = call.layer.unwrap_or(0);
            let request = DispatchRequest {
                mode: mode as i32,
                profile,
                limits: None,
                request_wire: call.request,
                proof: Some(WireProof {
                    profile,
                    method: call.semantic.get(),
                    wire_id: call.wire_id,
                    wire_size: call.wire_size as i64,
                    wire_digest: hex_lower(&call.wire_digest),
                    wire_invariant: call.wire_invariant,
                    effective_profile: call.effective_profile.map(|value| ProfileMark {
                        profile: value,
                        present: true,
                    }),
                    profile_evidence: call.profile_evidence.map(|value| ProfileMark {
                        profile: value,
                        present: true,
                    }),
                }),
                auth_key_id: call.auth_key_id.0.to_vec(),
                session_id: call.session.0,
                msg_id: call.msg_id,
                admission_seq: 0,
                profile_evidence_fresh: false,
                identity_hint: None,
                client_ip: String::new(),
            };
            let response: DispatchResponse =
                self.unary(request, methods::DISPATCH_ADMITTED).await?;
            if let Some(error) = response.rpc_error {
                return Err(RtdError::new(error.code, error.message));
            }
            if !response.error.is_empty() {
                return Err(RtdError::new(CORE_FAILURE_CODE, response.error));
            }
            Ok(CallResult {
                result: response.result_wire,
            })
        })
    }

    fn save_auth_key(&self, key: AuthKeyRecord) -> CoreFuture<'_, Result<(), RtdError>> {
        Box::pin(async move {
            let request = auth_key_to_proto(key);
            let response: ErrorResponse = self.unary(request, methods::SAVE_AUTH_KEY).await?;
            control_result(response)
        })
    }

    fn get_auth_key(
        &self,
        id: AuthKeyId,
    ) -> CoreFuture<'_, Result<Option<AuthKeyRecord>, RtdError>> {
        Box::pin(async move {
            let request = auth_key_request(id.0.to_vec(), 0, 0);
            let response: GetAuthKeyResponse = self.unary(request, methods::GET_AUTH_KEY).await?;
            if !response.error.is_empty() {
                return Err(RtdError::new(CORE_FAILURE_CODE, response.error));
            }
            if !response.found {
                return Ok(None);
            }
            let record = response.auth_key.ok_or_else(core_failure_error)?;
            let id = proto_auth_key_id(&record.auth_key_id)?;
            Ok(Some(AuthKeyRecord {
                id,
                dc: 0,
                key: record.value,
            }))
        })
    }

    fn resolve_layer_evidence(
        &self,
        session: SessionId,
    ) -> CoreFuture<'_, Result<Option<i32>, RtdError>> {
        Box::pin(async move {
            let request = auth_key_request(Vec::new(), session.0, 0);
            let response: ResolveSessionLayerResponse = self
                .unary(request, methods::RESOLVE_NEGOTIATED_SESSION_LAYER)
                .await?;
            if !response.error.is_empty() {
                return Err(RtdError::new(CORE_FAILURE_CODE, response.error));
            }
            Ok(if response.found {
                Some(response.layer)
            } else {
                None
            })
        })
    }

    fn advance_layer_evidence(
        &self,
        session: SessionId,
        layer: i32,
    ) -> CoreFuture<'_, Result<(), RtdError>> {
        Box::pin(async move {
            let request = auth_key_request(Vec::new(), session.0, layer);
            let response: AdvanceSessionLayerResponse = self
                .unary(request, methods::ADVANCE_NEGOTIATED_SESSION_LAYER)
                .await?;
            if response.error.is_empty() {
                Ok(())
            } else {
                Err(RtdError::new(CORE_FAILURE_CODE, response.error))
            }
        })
    }

    fn publish_layer_evidence(
        &self,
        evidence: LayerEvidence,
    ) -> CoreFuture<'_, Result<(), RtdError>> {
        Box::pin(async move {
            let request = PublishLayerEvidenceRequest {
                raw_auth_key_id: Vec::new(),
                session_id: evidence.session.0,
                msg_id: 0,
                admission_seq: 0,
                safe_floor: 0,
                layer: evidence.layer,
            };
            let response: ErrorResponse =
                self.unary(request, methods::PUBLISH_LAYER_EVIDENCE).await?;
            control_result(response)
        })
    }

    fn report_session_offline(
        &self,
        auth_key_id: AuthKeyId,
        session: SessionId,
    ) -> CoreFuture<'_, Result<(), RtdError>> {
        Box::pin(async move {
            let request = SessionOfflineBatchRequest {
                events: vec![SessionOfflineEvent {
                    raw_auth_key_id: auth_key_id.0.to_vec(),
                    session_id: session.0,
                    user_id: 0,
                    last_for_user: false,
                    disconnected_at: 0,
                }],
            };
            let response: ErrorResponse =
                self.unary(request, methods::REPORT_SESSION_OFFLINE).await?;
            control_result(response)
        })
    }
}

/// Lowercase hex encoding, matching gramsrv's `hex.EncodeToString`.
fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Maps an [`ErrorResponse`] control result to a TL result.
fn control_result(response: ErrorResponse) -> Result<(), RtdError> {
    if response.error.is_empty() {
        Ok(())
    } else {
        Err(RtdError::new(CORE_FAILURE_CODE, response.error))
    }
}

/// Builds an [`AuthKeyRequest`] addressing `raw_auth_key_id` at `session_id`.
fn auth_key_request(raw_auth_key_id: Vec<u8>, session_id: i64, layer: i32) -> AuthKeyRequest {
    AuthKeyRequest {
        raw_auth_key_id,
        session_id,
        layer,
        msg_id: 0,
        source_id: String::new(),
    }
}

/// Decodes a protobuf authorization-key id back to its fixed-size form.
fn proto_auth_key_id(bytes: &[u8]) -> Result<AuthKeyId, RtdError> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| core_failure_error())?;
    Ok(AuthKeyId(bytes))
}

/// Encodes an rtd-native authorization key as a protobuf record.
///
/// The coreexec contract has no datacenter field, so the edge-local `dc` is not
/// carried across the boundary.
fn auth_key_to_proto(key: AuthKeyRecord) -> ProtoAuthKeyRecord {
    ProtoAuthKeyRecord {
        auth_key_id: key.id.0.to_vec(),
        value: key.key,
        server_salt: 0,
        created_at: 0,
        expires_at: 0,
        layer: 0,
        layer_observation_id: 0,
        device_model: String::new(),
        platform: String::new(),
        system_version: String::new(),
        api_id: 0,
        app_version: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::future::Future;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::task::{Context, Poll};

    use rtd_bin::Writer;
    use rtd_edge_proto::{
        AdmissionMode, AdvanceSessionLayerResponse, AuthKeyRecord as ProtoAuthKeyRecord,
        AuthKeyRequest, CoreExecInfoRequest, CoreExecInfoResponse, DispatchRequest,
        DispatchResponse, ErrorResponse, GetAuthKeyResponse, PublishLayerEvidenceRequest, RPCError,
        ResolveSessionLayerResponse, SessionOfflineBatchRequest,
    };
    use rtd_server::core::{
        AdmittedCall, AuthKeyId, AuthKeyRecord, Core, LayerEvidence, LocalCore, SessionId,
    };
    use rtd_server::rpc::{DataCenter, HandlerRouter, ServerConfig};
    use rtd_tg::{HelpGetConfig, Method, Object};
    use rtd_tlprofile::semantic::METHOD_HELP_GET_CONFIG;
    use rtd_tlprofile::{Limits, Profile, SemanticId, decode_object};
    use tonic::codec::ProstCodec;
    use tonic::codegen::http;
    use tonic::transport::{Endpoint, Server};
    use tonic::{Request, Response, Status};

    use super::{GrpcCore, apply_bearer};

    /// A unary service built from a handler closure.
    struct UnaryServiceFn<M1, M2, F> {
        handler: F,
        _marker: std::marker::PhantomData<fn(M1) -> M2>,
    }

    impl<M1, M2, F> UnaryServiceFn<M1, M2, F> {
        fn new(handler: F) -> Self {
            Self {
                handler,
                _marker: std::marker::PhantomData,
            }
        }
    }

    impl<M1, M2, F, Fut> tonic::codegen::Service<Request<M1>> for UnaryServiceFn<M1, M2, F>
    where
        F: FnMut(Request<M1>) -> Fut,
        Fut: Future<Output = Result<Response<M2>, Status>>,
    {
        type Response = Response<M2>;
        type Error = Status;
        type Future = Fut;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: Request<M1>) -> Self::Future {
            (self.handler)(request)
        }
    }

    /// An in-process `CoreExecService` that delegates to a [`Core`].
    #[derive(Clone)]
    struct TestCoreService {
        core: Arc<dyn Core>,
    }

    impl tonic::server::NamedService for TestCoreService {
        const NAME: &'static str = rtd_edge_proto::SERVICE_NAME;
    }

    impl<B> tonic::codegen::Service<http::Request<B>> for TestCoreService
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Infallible;
        type Future = tonic::codegen::BoxFuture<Self::Response, Self::Error>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let core = Arc::clone(&self.core);
            let path = req.uri().path().to_owned();
            Box::pin(async move {
                let response = match path.as_str() {
                    rtd_edge_proto::methods::GET_INFO => serve_get_info(req).await,
                    rtd_edge_proto::methods::DISPATCH_ADMITTED => serve_dispatch(core, req).await,
                    rtd_edge_proto::methods::SAVE_AUTH_KEY => serve_save_auth_key(core, req).await,
                    rtd_edge_proto::methods::GET_AUTH_KEY => serve_get_auth_key(core, req).await,
                    rtd_edge_proto::methods::DELETE_AUTH_KEY => {
                        serve_delete_auth_key(core, req).await
                    }
                    rtd_edge_proto::methods::RESOLVE_NEGOTIATED_SESSION_LAYER => {
                        serve_resolve_layer(core, req).await
                    }
                    rtd_edge_proto::methods::ADVANCE_NEGOTIATED_SESSION_LAYER => {
                        serve_advance_layer(core, req).await
                    }
                    rtd_edge_proto::methods::PUBLISH_LAYER_EVIDENCE => {
                        serve_publish_evidence(core, req).await
                    }
                    rtd_edge_proto::methods::REPORT_SESSION_OFFLINE => {
                        serve_report_offline(core, req).await
                    }
                    _ => Status::unimplemented("unknown method").into_http(),
                };
                Ok(response)
            })
        }
    }

    async fn serve_get_info<B>(req: http::Request<B>) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        let service = UnaryServiceFn::<CoreExecInfoRequest, CoreExecInfoResponse, _>::new(
            |request: Request<CoreExecInfoRequest>| async move {
                // Echo the inbound authorization metadata as a capability so a
                // test can observe what the edge presented.
                let auth = request
                    .metadata()
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                let request = request.into_inner();
                let mut response = match rtd_edge_proto::handshake(&request) {
                    Ok(response) => response,
                    Err(error) => CoreExecInfoResponse {
                        protocol_version: 0,
                        min_supported_protocol_version: 0,
                        capabilities: Vec::new(),
                        implementation: String::new(),
                        error: error.message,
                        instance_id: String::new(),
                    },
                };
                if let Some(auth) = auth {
                    response.capabilities.push(format!("auth:{auth}"));
                }
                Ok::<_, Status>(Response::new(response))
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    async fn serve_dispatch<B>(
        core: Arc<dyn Core>,
        req: http::Request<B>,
    ) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        let service = UnaryServiceFn::<DispatchRequest, DispatchResponse, _>::new(
            move |request: Request<DispatchRequest>| {
                let core = Arc::clone(&core);
                async move {
                    let request = request.into_inner();
                    // The real core rejects a call without a resolved auth key;
                    // mirror that so a missing binding is visible in tests.
                    let auth_key_id =
                        parse_auth_key_id(&request.auth_key_id).ok_or_else(bad_auth_key_id)?;
                    let layer = if request.mode == AdmissionMode::Unprofiled as i32 {
                        None
                    } else {
                        Some(request.profile)
                    };
                    let (semantic, wire_id) = match request.proof.as_ref() {
                        Some(proof) => (SemanticId(proof.method), proof.wire_id),
                        None => (SemanticId(0), 0),
                    };
                    let wire_size = request.request_wire.len();
                    let call = AdmittedCall {
                        auth_key_id,
                        session: SessionId(request.session_id),
                        msg_id: request.msg_id,
                        semantic,
                        layer,
                        wire_id,
                        wire_size,
                        wire_digest: [0; 32],
                        wire_invariant: false,
                        effective_profile: None,
                        profile_evidence: None,
                        request: request.request_wire,
                    };
                    let response = match core.dispatch(call).await {
                        Ok(result) => DispatchResponse {
                            method: String::new(),
                            result_wire: result.result,
                            wire_invariant: false,
                            rpc_error: None,
                            error: String::new(),
                            post_response_actions: Vec::new(),
                        },
                        Err(error) => DispatchResponse {
                            method: String::new(),
                            result_wire: Vec::new(),
                            wire_invariant: false,
                            rpc_error: Some(RPCError {
                                code: error.code(),
                                message: error.message().to_owned(),
                            }),
                            error: String::new(),
                            post_response_actions: Vec::new(),
                        },
                    };
                    Ok::<_, Status>(Response::new(response))
                }
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    async fn serve_save_auth_key<B>(
        core: Arc<dyn Core>,
        req: http::Request<B>,
    ) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        let service = UnaryServiceFn::<ProtoAuthKeyRecord, ErrorResponse, _>::new(
            move |request: Request<ProtoAuthKeyRecord>| {
                let core = Arc::clone(&core);
                async move {
                    let record = request.into_inner();
                    let id = parse_auth_key_id(&record.auth_key_id).ok_or_else(bad_auth_key_id)?;
                    let native = AuthKeyRecord {
                        id,
                        dc: 0,
                        key: record.value,
                    };
                    let response = match core.save_auth_key(native).await {
                        Ok(()) => ErrorResponse {
                            error: String::new(),
                        },
                        Err(error) => ErrorResponse {
                            error: error.message().to_owned(),
                        },
                    };
                    Ok::<_, Status>(Response::new(response))
                }
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    async fn serve_get_auth_key<B>(
        core: Arc<dyn Core>,
        req: http::Request<B>,
    ) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        let service = UnaryServiceFn::<AuthKeyRequest, GetAuthKeyResponse, _>::new(
            move |request: Request<AuthKeyRequest>| {
                let core = Arc::clone(&core);
                async move {
                    let request = request.into_inner();
                    let id =
                        parse_auth_key_id(&request.raw_auth_key_id).ok_or_else(bad_auth_key_id)?;
                    let response = match core.get_auth_key(id).await {
                        Ok(Some(record)) => GetAuthKeyResponse {
                            auth_key: Some(ProtoAuthKeyRecord {
                                auth_key_id: record.id.0.to_vec(),
                                value: record.key,
                                server_salt: 0,
                                created_at: 0,
                                expires_at: 0,
                                layer: 0,
                                layer_observation_id: 0,
                                device_model: String::new(),
                                platform: String::new(),
                                system_version: String::new(),
                                api_id: 0,
                                app_version: String::new(),
                            }),
                            found: true,
                            error: String::new(),
                        },
                        Ok(None) => GetAuthKeyResponse {
                            auth_key: None,
                            found: false,
                            error: String::new(),
                        },
                        Err(error) => GetAuthKeyResponse {
                            auth_key: None,
                            found: false,
                            error: error.message().to_owned(),
                        },
                    };
                    Ok::<_, Status>(Response::new(response))
                }
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    async fn serve_delete_auth_key<B>(
        _core: Arc<dyn Core>,
        req: http::Request<B>,
    ) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        // The edge-facing `Core` trait has no delete operation, so this test
        // double validates the identifier and acknowledges the RPC.
        let service = UnaryServiceFn::<AuthKeyRequest, ErrorResponse, _>::new(
            |request: Request<AuthKeyRequest>| async move {
                let request = request.into_inner();
                let _id =
                    parse_auth_key_id(&request.raw_auth_key_id).ok_or_else(bad_auth_key_id)?;
                Ok::<_, Status>(Response::new(ErrorResponse {
                    error: String::new(),
                }))
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    async fn serve_resolve_layer<B>(
        core: Arc<dyn Core>,
        req: http::Request<B>,
    ) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        let service = UnaryServiceFn::<AuthKeyRequest, ResolveSessionLayerResponse, _>::new(
            move |request: Request<AuthKeyRequest>| {
                let core = Arc::clone(&core);
                async move {
                    let request = request.into_inner();
                    let response = match core
                        .resolve_layer_evidence(SessionId(request.session_id))
                        .await
                    {
                        Ok(Some(layer)) => ResolveSessionLayerResponse {
                            layer,
                            msg_id: 0,
                            found: true,
                            error: String::new(),
                        },
                        Ok(None) => ResolveSessionLayerResponse {
                            layer: 0,
                            msg_id: 0,
                            found: false,
                            error: String::new(),
                        },
                        Err(error) => ResolveSessionLayerResponse {
                            layer: 0,
                            msg_id: 0,
                            found: false,
                            error: error.message().to_owned(),
                        },
                    };
                    Ok::<_, Status>(Response::new(response))
                }
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    async fn serve_advance_layer<B>(
        core: Arc<dyn Core>,
        req: http::Request<B>,
    ) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        let service = UnaryServiceFn::<AuthKeyRequest, AdvanceSessionLayerResponse, _>::new(
            move |request: Request<AuthKeyRequest>| {
                let core = Arc::clone(&core);
                async move {
                    let request = request.into_inner();
                    let layer = request.layer;
                    let response = match core
                        .advance_layer_evidence(SessionId(request.session_id), layer)
                        .await
                    {
                        Ok(()) => AdvanceSessionLayerResponse {
                            current_layer: layer,
                            current_msg_id: 0,
                            publish_shared: false,
                            error: String::new(),
                        },
                        Err(error) => AdvanceSessionLayerResponse {
                            current_layer: 0,
                            current_msg_id: 0,
                            publish_shared: false,
                            error: error.message().to_owned(),
                        },
                    };
                    Ok::<_, Status>(Response::new(response))
                }
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    async fn serve_publish_evidence<B>(
        core: Arc<dyn Core>,
        req: http::Request<B>,
    ) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        let service = UnaryServiceFn::<PublishLayerEvidenceRequest, ErrorResponse, _>::new(
            move |request: Request<PublishLayerEvidenceRequest>| {
                let core = Arc::clone(&core);
                async move {
                    let request = request.into_inner();
                    let evidence = LayerEvidence {
                        session: SessionId(request.session_id),
                        layer: request.layer,
                    };
                    let response = match core.publish_layer_evidence(evidence).await {
                        Ok(()) => ErrorResponse {
                            error: String::new(),
                        },
                        Err(error) => ErrorResponse {
                            error: error.message().to_owned(),
                        },
                    };
                    Ok::<_, Status>(Response::new(response))
                }
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    async fn serve_report_offline<B>(
        core: Arc<dyn Core>,
        req: http::Request<B>,
    ) -> http::Response<tonic::body::BoxBody>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<tonic::codegen::StdError> + Send + 'static,
    {
        let service = UnaryServiceFn::<SessionOfflineBatchRequest, ErrorResponse, _>::new(
            move |request: Request<SessionOfflineBatchRequest>| {
                let core = Arc::clone(&core);
                async move {
                    let request = request.into_inner();
                    for event in request.events {
                        let Some(auth_key_id) = parse_auth_key_id(&event.raw_auth_key_id) else {
                            return Ok::<_, Status>(Response::new(ErrorResponse {
                                error: "bad raw auth key".to_owned(),
                            }));
                        };
                        if let Err(error) = core
                            .report_session_offline(auth_key_id, SessionId(event.session_id))
                            .await
                        {
                            return Ok::<_, Status>(Response::new(ErrorResponse {
                                error: error.message().to_owned(),
                            }));
                        }
                    }
                    Ok::<_, Status>(Response::new(ErrorResponse {
                        error: String::new(),
                    }))
                }
            },
        );
        let mut grpc = tonic::server::Grpc::new(ProstCodec::default());
        grpc.unary(service, req).await
    }

    fn parse_auth_key_id(bytes: &[u8]) -> Option<AuthKeyId> {
        let bytes: [u8; 8] = bytes.try_into().ok()?;
        // An all-zero id is the unencrypted marker, never a real auth key.
        if bytes == [0u8; 8] {
            return None;
        }
        Some(AuthKeyId(bytes))
    }

    fn bad_auth_key_id() -> Status {
        Status::invalid_argument("bad auth_key_id")
    }

    async fn spawn_core(core: Arc<dyn Core>) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");
        let incoming = tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener);
        let handle = tokio::spawn(async move {
            let _ = Server::builder()
                .add_service(TestCoreService { core })
                .serve_with_incoming(incoming)
                .await;
        });
        (addr, handle)
    }

    async fn connect_adapter(addr: SocketAddr) -> GrpcCore {
        let endpoint = Endpoint::from_shared(format!("http://{addr}")).expect("valid endpoint");
        GrpcCore::connect(endpoint).await.expect("connect adapter")
    }

    fn server_config() -> ServerConfig {
        ServerConfig::new(2, vec![DataCenter::new(2, "127.0.0.1", 443)])
    }

    fn canonical_call() -> AdmittedCall {
        let mut writer = Writer::new();
        Method::HelpGetConfig(Box::new(HelpGetConfig)).encode(&mut writer);
        let request = writer.into_vec();
        AdmittedCall {
            auth_key_id: AuthKeyId([9, 9, 9, 9, 9, 9, 9, 9]),
            session: SessionId(7),
            msg_id: 11,
            semantic: METHOD_HELP_GET_CONFIG,
            layer: Some(232),
            wire_id: HelpGetConfig::TYPE_ID,
            wire_size: request.len(),
            wire_digest: [0; 32],
            wire_invariant: true,
            effective_profile: Some(232),
            profile_evidence: None,
            request,
        }
    }

    #[tokio::test]
    async fn info_negotiates_over_grpc() {
        let core: Arc<dyn Core> = Arc::new(LocalCore::with_builtins(&server_config()));
        let (addr, _handle) = spawn_core(core).await;
        let adapter = connect_adapter(addr).await;
        let info = adapter.info().await.expect("info");
        assert_eq!(info.protocol_version, rtd_edge_proto::PROTOCOL_VERSION);
        assert_eq!(info.dc, 0);
    }

    #[test]
    fn apply_bearer_sets_and_omits_token() {
        let mut with = Request::new(());
        apply_bearer(&mut with, Some("s3cret")).expect("token");
        assert_eq!(
            with.metadata().get("authorization").expect("header"),
            "Bearer s3cret"
        );

        let mut without = Request::new(());
        apply_bearer(&mut without, None).expect("no token");
        assert!(without.metadata().get("authorization").is_none());
    }

    #[tokio::test]
    async fn core_exec_token_is_presented_as_bearer_metadata() {
        let core: Arc<dyn Core> = Arc::new(LocalCore::with_builtins(&server_config()));
        let (addr, _handle) = spawn_core(core).await;
        let endpoint = Endpoint::from_shared(format!("http://{addr}")).expect("valid endpoint");
        let adapter = GrpcCore::connect(endpoint)
            .await
            .expect("connect adapter")
            .with_token("s3cret");
        let info = adapter.info().await.expect("info");
        assert!(
            info.capabilities
                .iter()
                .any(|name| name == "auth:Bearer s3cret"),
            "capabilities were {:?}",
            info.capabilities
        );
    }

    #[tokio::test]
    async fn no_token_sends_no_authorization_metadata() {
        let core: Arc<dyn Core> = Arc::new(LocalCore::with_builtins(&server_config()));
        let (addr, _handle) = spawn_core(core).await;
        let adapter = connect_adapter(addr).await;
        let info = adapter.info().await.expect("info");
        assert!(
            !info
                .capabilities
                .iter()
                .any(|name| name.starts_with("auth:")),
            "capabilities were {:?}",
            info.capabilities
        );
    }

    #[tokio::test]
    async fn dispatch_round_trips_over_grpc() {
        let core: Arc<dyn Core> = Arc::new(LocalCore::with_builtins(&server_config()));
        let (addr, _handle) = spawn_core(core).await;
        let adapter = connect_adapter(addr).await;

        let result = adapter.dispatch(canonical_call()).await.expect("dispatch");

        // The remote result is byte-identical to the in-process core's.
        let local = LocalCore::with_builtins(&server_config());
        let expected = local
            .dispatch(canonical_call())
            .await
            .expect("local dispatch");
        assert_eq!(result.result, expected.result);

        let decoded = decode_object(Profile::new(232), &result.result, Limits::default())
            .expect("decode result");
        let Object::Config(config) = decoded else {
            panic!("help.getConfig did not return a Config");
        };
        assert_eq!(config.this_dc, 2);
    }

    #[tokio::test]
    async fn dispatch_without_auth_key_id_fails() {
        // The mock core rejects an empty auth_key_id (as gramsrv v2 does), so a
        // call that lost its binding fails rather than being served unbound.
        let core: Arc<dyn Core> = Arc::new(LocalCore::with_builtins(&server_config()));
        let (addr, _handle) = spawn_core(core).await;
        let adapter = connect_adapter(addr).await;

        let mut call = canonical_call();
        call.auth_key_id = AuthKeyId([0; 8]);
        let error = adapter
            .dispatch(call)
            .await
            .expect_err("an unbound call is rejected");
        assert_eq!(error.message(), "CORE_FAILURE");
    }

    #[tokio::test]
    async fn core_error_maps_to_tl_error() {
        let core: Arc<dyn Core> = Arc::new(LocalCore::with_builtins(&server_config()));
        let (addr, _handle) = spawn_core(core).await;
        let adapter = connect_adapter(addr).await;

        let mut call = canonical_call();
        call.request = vec![0xff, 0xff, 0xff, 0xff];
        let error = adapter.dispatch(call).await.expect_err("core failure");
        assert_eq!(error.code(), 400);
        assert_eq!(error.message(), "REQUEST_DECODE_FAILED");
    }

    #[tokio::test]
    async fn auth_key_round_trips_over_grpc() {
        let core: Arc<dyn Core> = Arc::new(LocalCore::new(HandlerRouter::new(), 2));
        let (addr, _handle) = spawn_core(core).await;
        let adapter = connect_adapter(addr).await;

        let id = AuthKeyId([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(adapter.get_auth_key(id).await.expect("get"), None);
        adapter
            .save_auth_key(AuthKeyRecord {
                id,
                dc: 2,
                key: vec![0xaa; 32],
            })
            .await
            .expect("save");
        let stored = adapter
            .get_auth_key(id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(stored.id, id);
        assert_eq!(stored.key, vec![0xaa; 32]);
        assert!(adapter.delete_auth_key(id).await.is_ok());
    }
}
