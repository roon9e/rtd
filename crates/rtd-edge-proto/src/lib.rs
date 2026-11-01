//! The gRPC contract between an rtd MTProto edge and a core service.
//!
//! The authoritative definition is `proto/edge.proto`, a faithful subset of
//! gramsrv v2's `internal/coreexec/coreexecpb/coreexec.proto`. Reproducing the
//! upstream package (`telesrv.coreexec.v1`), service ([`CoreExecService`]),
//! method paths and message field tags is what lets an rtd edge front an existing
//! gramsrv core unchanged: gRPC method paths are `/<package>.<service>/<method>`,
//! so the name is the contract.
//!
//! The message types below are the committed prost form of that schema. They are
//! hand-written against the pinned upstream file (see [`provenance`]); no
//! `tonic-build` runs at build time. The protocol is versioned: the edge speaks
//! [`PROTOCOL_VERSION`] and [`handshake`]/[`verify_core_info`] reject an
//! incompatible version or a core missing a [`REQUIRED_CAPABILITIES`] entry
//! rather than misbehaving.
#![warn(missing_docs)]

/// The protocol version this crate speaks.
///
/// Mirrors gramsrv v2's `coreExecGRPCProtocolVersion`.
pub const PROTOCOL_VERSION: u32 = 5;

/// The lowest protocol version this crate supports.
///
/// Mirrors gramsrv v2's `coreExecGRPCMinSupportedProtocolVersion`.
pub const MIN_SUPPORTED_PROTOCOL_VERSION: u32 = 5;

/// The protobuf package of the boundary contract.
pub const PACKAGE: &str = "telesrv.coreexec.v1";

/// The fully-qualified gRPC service name of the boundary contract.
pub const SERVICE_NAME: &str = "telesrv.coreexec.v1.CoreExecService";

/// The capabilities this crate advertises to a core.
pub const CAPABILITIES: &[&str] = &[
    "tl-wire-passthrough",
    "updates-lifecycle-fencing",
    "dispatch-admitted",
    "profile-evidence",
    "identity-hint",
    "session-updates-readiness-hint",
    "session-offline-batch",
    "init-connection-observer",
    "replay-prepare-commit",
    "auth-key-authority",
];

/// The capabilities this crate requires a core to advertise.
///
/// Mirrors gramsrv v2's `coreExecGRPCRequiredCapabilities`.
pub const REQUIRED_CAPABILITIES: &[&str] = &[
    "auth-key-authority",
    "session-offline-batch",
    "session-updates-readiness-hint",
    "updates-lifecycle-fencing",
];

/// The provenance of the pinned upstream contract.
///
/// The boundary is a *pin*, not a fork: the upstream schema revision is recorded
/// here, and [`SOURCE_SHA256`](provenance::SOURCE_SHA256) is the digest of the
/// exact upstream bytes, so the mirror cannot drift silently and can be upgraded
/// deliberately.
pub mod provenance {
    /// The upstream remote-tracking ref the contract was read from.
    pub const SOURCE_REF: &str = "origin/v2";

    /// The upstream commit of the pinned contract.
    pub const SOURCE_COMMIT: &str = "c9cc874f096e0a21d53c94288a4ba6bcb24fd294";

    /// The path of the pinned file within the upstream repository.
    pub const SOURCE_PATH: &str = "internal/coreexec/coreexecpb/coreexec.proto";

    /// The git blob hash of the exact upstream file bytes.
    pub const SOURCE_BLOB: &str = "e545a70f702171a3359d7e1231e873ae70d4f995";

    /// The lowercase-hex SHA-256 of the exact upstream file bytes.
    pub const SOURCE_SHA256: &str =
        "0e177e856541768a22fb986b0647b1e2edb22cb124c7e37c0c11bfc682abdcb4";
}

/// A one-line provenance record for the pinned contract.
pub const PROVENANCE: &str = "gramsrv origin/v2 @ c9cc874f096e0a21d53c94288a4ba6bcb24fd294 \
     internal/coreexec/coreexecpb/coreexec.proto \
     sha256:0e177e856541768a22fb986b0647b1e2edb22cb124c7e37c0c11bfc682abdcb4";

/// The fully-qualified gRPC method paths of the `CoreExecService` subset the edge
/// uses.
pub mod methods {
    /// `CoreExecService.GetInfo` — protocol and capability negotiation.
    pub const GET_INFO: &str = "/telesrv.coreexec.v1.CoreExecService/GetInfo";

    /// `CoreExecService.DispatchAdmitted` — execute one admitted call.
    pub const DISPATCH_ADMITTED: &str = "/telesrv.coreexec.v1.CoreExecService/DispatchAdmitted";

    /// `CoreExecService.SaveAuthKey` — durably persist an authorization key.
    pub const SAVE_AUTH_KEY: &str = "/telesrv.coreexec.v1.CoreExecService/SaveAuthKey";

    /// `CoreExecService.GetAuthKey` — resolve an authorization key.
    pub const GET_AUTH_KEY: &str = "/telesrv.coreexec.v1.CoreExecService/GetAuthKey";

    /// `CoreExecService.DeleteAuthKey` — remove an authorization key.
    pub const DELETE_AUTH_KEY: &str = "/telesrv.coreexec.v1.CoreExecService/DeleteAuthKey";

    /// `CoreExecService.UpdateAuthKeyClientInfo` — refresh client metadata.
    pub const UPDATE_AUTH_KEY_CLIENT_INFO: &str =
        "/telesrv.coreexec.v1.CoreExecService/UpdateAuthKeyClientInfo";

    /// `CoreExecService.ResolveNegotiatedSessionLayer` — read session-layer evidence.
    pub const RESOLVE_NEGOTIATED_SESSION_LAYER: &str =
        "/telesrv.coreexec.v1.CoreExecService/ResolveNegotiatedSessionLayer";

    /// `CoreExecService.ResolveInheritedAuthKeyLayer` — read the auth key's default layer.
    pub const RESOLVE_INHERITED_AUTH_KEY_LAYER: &str =
        "/telesrv.coreexec.v1.CoreExecService/ResolveInheritedAuthKeyLayer";

    /// `CoreExecService.AdvanceNegotiatedSessionLayer` — advance session-layer evidence.
    pub const ADVANCE_NEGOTIATED_SESSION_LAYER: &str =
        "/telesrv.coreexec.v1.CoreExecService/AdvanceNegotiatedSessionLayer";

    /// `CoreExecService.DeleteNegotiatedSessionLayer` — drop session-layer evidence.
    pub const DELETE_NEGOTIATED_SESSION_LAYER: &str =
        "/telesrv.coreexec.v1.CoreExecService/DeleteNegotiatedSessionLayer";

    /// `CoreExecService.PublishLayerEvidence` — publish admitted layer evidence.
    pub const PUBLISH_LAYER_EVIDENCE: &str =
        "/telesrv.coreexec.v1.CoreExecService/PublishLayerEvidence";

    /// `CoreExecService.ObserveInitConnection` — record an initialization connection.
    pub const OBSERVE_INIT_CONNECTION: &str =
        "/telesrv.coreexec.v1.CoreExecService/ObserveInitConnection";

    /// `CoreExecService.ReportSessionOffline` — report offline sessions in a batch.
    pub const REPORT_SESSION_OFFLINE: &str =
        "/telesrv.coreexec.v1.CoreExecService/ReportSessionOffline";
}

/// Failure returned when a protocol handshake cannot be completed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeError {
    /// A human-readable diagnosis of the mismatch.
    pub message: String,
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for HandshakeError {}

/// Returns whether two inclusive protocol-version ranges overlap.
///
/// A zero maximum means "no versions"; a zero minimum falls back to the maximum,
/// matching gramsrv v2's `coreExecProtocolRangesOverlap`.
#[must_use]
pub fn protocol_ranges_overlap(
    peer_min: u32,
    peer_max: u32,
    local_min: u32,
    local_max: u32,
) -> bool {
    if peer_max == 0 || local_max == 0 {
        return false;
    }
    let peer_min = if peer_min == 0 { peer_max } else { peer_min };
    let local_min = if local_min == 0 { local_max } else { local_min };
    peer_max >= local_min && local_max >= peer_min
}

/// Returns whether `version` is the protocol version this crate speaks.
#[must_use]
pub fn version_is_supported(version: u32) -> bool {
    protocol_ranges_overlap(
        version,
        version,
        MIN_SUPPORTED_PROTOCOL_VERSION,
        PROTOCOL_VERSION,
    )
}

/// Returns the [`REQUIRED_CAPABILITIES`] absent from `advertised`.
#[must_use]
pub fn missing_capabilities(advertised: &[String]) -> Vec<&'static str> {
    REQUIRED_CAPABILITIES
        .iter()
        .copied()
        .filter(|required| !advertised.iter().any(|have| have.as_str() == *required))
        .collect()
}

/// Builds this crate's [`CoreExecInfoResponse`] for a caller's
/// [`CoreExecInfoRequest`].
///
/// This is the `GetInfo` answer of a peer that implements the contract: it
/// advertises [`PROTOCOL_VERSION`], [`MIN_SUPPORTED_PROTOCOL_VERSION`] and
/// [`CAPABILITIES`].
///
/// # Errors
///
/// Returns a [`HandshakeError`] when the caller's range and this crate's
/// `MIN_SUPPORTED_PROTOCOL_VERSION..=PROTOCOL_VERSION` do not overlap, rejecting
/// an unsupported version instead of misbehaving.
pub fn handshake(request: &CoreExecInfoRequest) -> Result<CoreExecInfoResponse, HandshakeError> {
    if !protocol_ranges_overlap(
        request.min_supported_protocol_version,
        request.protocol_version,
        MIN_SUPPORTED_PROTOCOL_VERSION,
        PROTOCOL_VERSION,
    ) {
        return Err(HandshakeError {
            message: format!(
                "incompatible protocol peer={}..{} local={}..{}",
                request.min_supported_protocol_version,
                request.protocol_version,
                MIN_SUPPORTED_PROTOCOL_VERSION,
                PROTOCOL_VERSION,
            ),
        });
    }
    Ok(CoreExecInfoResponse {
        protocol_version: PROTOCOL_VERSION,
        min_supported_protocol_version: MIN_SUPPORTED_PROTOCOL_VERSION,
        capabilities: CAPABILITIES.iter().map(|name| (*name).to_owned()).collect(),
        implementation: "rtd-edge".to_owned(),
        error: String::new(),
        instance_id: String::new(),
    })
}

/// Validates a core's [`CoreExecInfoResponse`]: the protocol ranges must overlap
/// and every [`REQUIRED_CAPABILITIES`] entry must be advertised.
///
/// # Errors
///
/// Returns a [`HandshakeError`] naming the protocol range or capabilities that
/// make the core unusable.
pub fn verify_core_info(response: &CoreExecInfoResponse) -> Result<(), HandshakeError> {
    if !protocol_ranges_overlap(
        response.min_supported_protocol_version,
        response.protocol_version,
        MIN_SUPPORTED_PROTOCOL_VERSION,
        PROTOCOL_VERSION,
    ) {
        return Err(HandshakeError {
            message: format!(
                "incompatible protocol core={}..{} local={}..{}",
                response.min_supported_protocol_version,
                response.protocol_version,
                MIN_SUPPORTED_PROTOCOL_VERSION,
                PROTOCOL_VERSION,
            ),
        });
    }
    let missing = missing_capabilities(&response.capabilities);
    if !missing.is_empty() {
        return Err(HandshakeError {
            message: format!("missing capabilities {}", missing.join(",")),
        });
    }
    Ok(())
}

/// Protocol and capability negotiation request of `CoreExecService.GetInfo`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct CoreExecInfoRequest {
    /// The protocol version the caller speaks.
    #[prost(uint32, tag = "1")]
    pub protocol_version: u32,
    /// The lowest protocol version the caller supports.
    #[prost(uint32, tag = "2")]
    pub min_supported_protocol_version: u32,
    /// The capabilities the caller advertises.
    #[prost(string, repeated, tag = "3")]
    pub capabilities: Vec<String>,
}

/// Protocol and capability negotiation response of `CoreExecService.GetInfo`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct CoreExecInfoResponse {
    /// The protocol version the responder speaks.
    #[prost(uint32, tag = "1")]
    pub protocol_version: u32,
    /// The lowest protocol version the responder supports.
    #[prost(uint32, tag = "2")]
    pub min_supported_protocol_version: u32,
    /// The capabilities the responder advertises.
    #[prost(string, repeated, tag = "3")]
    pub capabilities: Vec<String>,
    /// A diagnostic identifier for the responder implementation.
    #[prost(string, tag = "4")]
    pub implementation: String,
    /// A non-empty diagnosis when negotiation failed.
    #[prost(string, tag = "5")]
    pub error: String,
    /// The responder instance id, for diagnostics.
    #[prost(string, tag = "6")]
    pub instance_id: String,
}

/// How a [`DispatchRequest`] was admitted at the edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AdmissionMode {
    /// The mode was not specified.
    Unspecified = 0,
    /// Admission bound the call to a negotiated layer.
    Layer = 1,
    /// Admission used the default profile.
    Default = 2,
    /// Admission used an unprofiled route.
    Unprofiled = 3,
}

/// Decode and admission limits attached to a [`DispatchRequest`].
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct Limits {
    /// Maximum accepted wire size in bytes.
    #[prost(int64, tag = "1")]
    pub max_wire_bytes: i64,
    /// Maximum elements in a single vector.
    #[prost(int64, tag = "2")]
    pub max_vector_elements: i64,
    /// Maximum aggregate vector elements.
    #[prost(int64, tag = "3")]
    pub max_aggregate_elements: i64,
    /// Maximum nesting depth.
    #[prost(int64, tag = "4")]
    pub max_depth: i64,
}

/// The presence and value of one profile mark.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct ProfileMark {
    /// The marked profile number.
    #[prost(int32, tag = "1")]
    pub profile: i32,
    /// Whether the mark was present on the wire.
    #[prost(bool, tag = "2")]
    pub present: bool,
}

/// The edge's proof of the admitted call's identity and wire shape.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct WireProof {
    /// The profile the proof was taken in.
    #[prost(int32, tag = "1")]
    pub profile: i32,
    /// The semantic method id.
    #[prost(uint64, tag = "2")]
    pub method: u64,
    /// The wire constructor id.
    #[prost(uint32, tag = "3")]
    pub wire_id: u32,
    /// The wire size in bytes.
    #[prost(int64, tag = "4")]
    pub wire_size: i64,
    /// The digest of the wire bytes.
    #[prost(string, tag = "5")]
    pub wire_digest: String,
    /// Whether the encoding was wire-invariant.
    #[prost(bool, tag = "6")]
    pub wire_invariant: bool,
    /// The effective profile mark.
    #[prost(message, optional, tag = "7")]
    pub effective_profile: Option<ProfileMark>,
    /// The profile-evidence mark.
    #[prost(message, optional, tag = "8")]
    pub profile_evidence: Option<ProfileMark>,
}

/// A TL error surfaced to the client.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct RPCError {
    /// The TL error code.
    #[prost(int32, tag = "1")]
    pub code: i32,
    /// The TL error message.
    #[prost(string, tag = "2")]
    pub message: String,
}

/// Edge-supplied identity hints for an admitted call.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct IdentityHint {
    /// The resolved business authorization-key id.
    #[prost(bytes = "vec", tag = "1")]
    pub business_auth_key_id: Vec<u8>,
    /// Whether `business_auth_key_id` is resolved.
    #[prost(bool, tag = "2")]
    pub business_auth_key_resolved: bool,
    /// The resolved user id.
    #[prost(int64, tag = "3")]
    pub user_id: i64,
    /// Whether `user_id` is resolved.
    #[prost(bool, tag = "4")]
    pub user_id_resolved: bool,
    /// The raw authorization-key expiry, as a Unix timestamp.
    #[prost(int32, tag = "5")]
    pub raw_auth_key_expires_at: i32,
    /// Whether `raw_auth_key_expires_at` is resolved.
    #[prost(bool, tag = "6")]
    pub raw_auth_key_expires_at_resolved: bool,
    /// Whether session updates are ready to be delivered.
    #[prost(bool, tag = "7")]
    pub session_updates_ready: bool,
}

/// One admitted call handed to the core by `CoreExecService.DispatchAdmitted`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct DispatchRequest {
    /// How the call was admitted.
    #[prost(enumeration = "AdmissionMode", tag = "1")]
    pub mode: i32,
    /// The negotiated profile number.
    #[prost(int32, tag = "2")]
    pub profile: i32,
    /// The admission limits.
    #[prost(message, optional, tag = "3")]
    pub limits: Option<Limits>,
    /// The raw TL request bytes.
    #[prost(bytes = "vec", tag = "4")]
    pub request_wire: Vec<u8>,
    /// The edge's wire proof.
    #[prost(message, optional, tag = "5")]
    pub proof: Option<WireProof>,
    /// The authorization-key id.
    #[prost(bytes = "vec", tag = "6")]
    pub auth_key_id: Vec<u8>,
    /// The MTProto session id.
    #[prost(int64, tag = "7")]
    pub session_id: i64,
    /// The message id.
    #[prost(int64, tag = "8")]
    pub msg_id: i64,
    /// The admission sequence number.
    #[prost(uint64, tag = "9")]
    pub admission_seq: u64,
    /// Whether the layer RPC profile evidence is fresh.
    #[prost(bool, tag = "10")]
    pub profile_evidence_fresh: bool,
    /// Edge-supplied identity hints.
    #[prost(message, optional, tag = "11")]
    pub identity_hint: Option<IdentityHint>,
    /// Edge-observed peer IP literal: canonical, without port or zone. Empty when
    /// the transport peer is not an IP.
    #[prost(string, tag = "12")]
    pub client_ip: String,
}

/// The core's answer to a [`DispatchRequest`].
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct DispatchResponse {
    /// The semantic method name that was executed.
    #[prost(string, tag = "1")]
    pub method: String,
    /// The raw TL result bytes, encoded by the edge per layer.
    #[prost(bytes = "vec", tag = "2")]
    pub result_wire: Vec<u8>,
    /// Whether the result encoding was wire-invariant.
    #[prost(bool, tag = "3")]
    pub wire_invariant: bool,
    /// A TL error to surface to the client.
    #[prost(message, optional, tag = "4")]
    pub rpc_error: Option<RPCError>,
    /// A control-plane error.
    #[prost(string, tag = "5")]
    pub error: String,
    /// Actions to run after the response is delivered.
    #[prost(message, repeated, tag = "6")]
    pub post_response_actions: Vec<PostResponseAction>,
}

/// A side effect the edge performs after delivering a response.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct PostResponseAction {
    /// The concrete action.
    #[prost(oneof = "post_response_action::Action", tags = "1, 2")]
    pub action: Option<post_response_action::Action>,
}

/// Nested message and enum types in [`PostResponseAction`].
pub mod post_response_action {
    /// The concrete post-response action.
    #[derive(Clone, PartialEq, Eq, ::prost::Oneof)]
    pub enum Action {
        /// Commit updates-delivery lifecycle transitions.
        #[prost(message, tag = "1")]
        UpdatesDelivery(super::UpdatesDeliveryAction),
        /// Tear down an account's authorization state.
        #[prost(message, tag = "2")]
        AccountAuthorizationTeardown(super::AccountAuthorizationTeardownAction),
    }
}

/// Tear down an account's authorization state after delivering a response.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct AccountAuthorizationTeardownAction {
    /// The user whose authorization is torn down.
    #[prost(int64, tag = "1")]
    pub user_id: i64,
    /// The authorization-key id to invalidate.
    #[prost(bytes = "vec", tag = "2")]
    pub auth_key_id: Vec<u8>,
}

/// A committed update-state cursor.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct UpdateState {
    /// The pts cursor.
    #[prost(int32, tag = "1")]
    pub pts: i32,
    /// The qts cursor.
    #[prost(int32, tag = "2")]
    pub qts: i32,
    /// The date cursor.
    #[prost(int32, tag = "3")]
    pub date: i32,
    /// The seq cursor.
    #[prost(int32, tag = "4")]
    pub seq: i32,
}

/// How an [`UpdatesDeliveryAction`] cursor is committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum UpdateStateCommitMode {
    /// The mode was not specified.
    Unspecified = 0,
    /// Commit the delivered cursor only.
    DeliveredOnly = 1,
    /// Commit the delivered and observed-baseline cursors.
    DeliveredAndObservedBaseline = 2,
}

/// Updates-delivery lifecycle transitions committed after a response.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct UpdatesDeliveryAction {
    /// Whether to commit the cursor.
    #[prost(bool, tag = "1")]
    pub commit_cursor: bool,
    /// The cursor's authorization-key id.
    #[prost(bytes = "vec", tag = "2")]
    pub cursor_auth_key_id: Vec<u8>,
    /// The cursor's user id.
    #[prost(int64, tag = "3")]
    pub cursor_user_id: i64,
    /// The cursor state.
    #[prost(message, optional, tag = "4")]
    pub cursor_state: Option<UpdateState>,
    /// The cursor commit mode.
    #[prost(enumeration = "UpdateStateCommitMode", tag = "5")]
    pub cursor_mode: i32,
    /// Whether to mark a secret delivered.
    #[prost(bool, tag = "6")]
    pub mark_secret_delivered: bool,
    /// The secret device key.
    #[prost(int64, tag = "7")]
    pub secret_device_key: i64,
    /// The delivered secret event ids.
    #[prost(int64, repeated, tag = "8")]
    pub secret_event_ids: Vec<i64>,
    /// Whether to mark the session ready.
    #[prost(bool, tag = "9")]
    pub mark_session_ready: bool,
    /// The ready raw authorization-key id.
    #[prost(bytes = "vec", tag = "10")]
    pub ready_raw_auth_key_id: Vec<u8>,
    /// The ready session id.
    #[prost(int64, tag = "11")]
    pub ready_session_id: i64,
    /// The ready user id.
    #[prost(int64, tag = "12")]
    pub ready_user_id: i64,
    /// Whether to publish a bootstrap.
    #[prost(bool, tag = "13")]
    pub publish_bootstrap: bool,
    /// The bootstrap user id.
    #[prost(int64, tag = "14")]
    pub bootstrap_user_id: i64,
    /// The activation token.
    #[prost(uint64, tag = "15")]
    pub activation_token: u64,
    /// The bootstrap authorization-key id.
    #[prost(bytes = "vec", tag = "16")]
    pub bootstrap_auth_key_id: Vec<u8>,
    /// The bootstrap session id.
    #[prost(int64, tag = "17")]
    pub bootstrap_session_id: i64,
    /// The bootstrap probe token.
    #[prost(uint64, tag = "18")]
    pub bootstrap_probe_token: u64,
    /// The bootstrap raw authorization-key id.
    #[prost(bytes = "vec", tag = "19")]
    pub bootstrap_raw_auth_key_id: Vec<u8>,
}

/// One offline session in a [`SessionOfflineBatchRequest`].
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct SessionOfflineEvent {
    /// The raw authorization-key id of the offline session.
    #[prost(bytes = "vec", tag = "1")]
    pub raw_auth_key_id: Vec<u8>,
    /// The MTProto session id.
    #[prost(int64, tag = "2")]
    pub session_id: i64,
    /// The user the session belonged to.
    #[prost(int64, tag = "3")]
    pub user_id: i64,
    /// Whether this was the user's last live session.
    #[prost(bool, tag = "4")]
    pub last_for_user: bool,
    /// When the session disconnected, as a Unix timestamp.
    #[prost(int32, tag = "5")]
    pub disconnected_at: i32,
}

/// A batch of offline sessions reported by `CoreExecService.ReportSessionOffline`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct SessionOfflineBatchRequest {
    /// The offline sessions.
    #[prost(message, repeated, tag = "1")]
    pub events: Vec<SessionOfflineEvent>,
}

/// The key addressed by the layer-evidence and auth-key RPCs.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct AuthKeyRequest {
    /// The raw authorization-key id.
    #[prost(bytes = "vec", tag = "1")]
    pub raw_auth_key_id: Vec<u8>,
    /// The MTProto session id, when the request is session-scoped.
    #[prost(int64, tag = "2")]
    pub session_id: i64,
    /// The layer, when the request advances one.
    #[prost(int32, tag = "3")]
    pub layer: i32,
    /// The message id, when the request advances one.
    #[prost(int64, tag = "4")]
    pub msg_id: i64,
    /// A diagnostics source identifier.
    #[prost(string, tag = "5")]
    pub source_id: String,
}

/// A durable authorization key.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct AuthKeyRecord {
    /// The authorization-key id.
    #[prost(bytes = "vec", tag = "1")]
    pub auth_key_id: Vec<u8>,
    /// The key material.
    #[prost(bytes = "vec", tag = "2")]
    pub value: Vec<u8>,
    /// The server salt.
    #[prost(int64, tag = "3")]
    pub server_salt: i64,
    /// When the key was created, as a Unix timestamp.
    #[prost(int64, tag = "4")]
    pub created_at: i64,
    /// When the key expires, as a Unix timestamp.
    #[prost(int32, tag = "5")]
    pub expires_at: i32,
    /// The key's default layer.
    #[prost(int32, tag = "6")]
    pub layer: i32,
    /// The layer-observation id.
    #[prost(int64, tag = "7")]
    pub layer_observation_id: i64,
    /// The client device model.
    #[prost(string, tag = "8")]
    pub device_model: String,
    /// The client platform.
    #[prost(string, tag = "9")]
    pub platform: String,
    /// The client system version.
    #[prost(string, tag = "10")]
    pub system_version: String,
    /// The client API id.
    #[prost(int32, tag = "11")]
    pub api_id: i32,
    /// The client app version.
    #[prost(string, tag = "12")]
    pub app_version: String,
}

/// The answer to `CoreExecService.GetAuthKey`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct GetAuthKeyResponse {
    /// The resolved key, when found.
    #[prost(message, optional, tag = "1")]
    pub auth_key: Option<AuthKeyRecord>,
    /// Whether the key was found.
    #[prost(bool, tag = "2")]
    pub found: bool,
    /// A control-plane error.
    #[prost(string, tag = "3")]
    pub error: String,
}

/// Client metadata refresh sent to `CoreExecService.UpdateAuthKeyClientInfo`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct AuthKeyClientInfoRequest {
    /// The authorization-key id.
    #[prost(bytes = "vec", tag = "1")]
    pub auth_key_id: Vec<u8>,
    /// The key's layer.
    #[prost(int32, tag = "2")]
    pub layer: i32,
    /// The client device model.
    #[prost(string, tag = "3")]
    pub device_model: String,
    /// The client platform.
    #[prost(string, tag = "4")]
    pub platform: String,
    /// The client system version.
    #[prost(string, tag = "5")]
    pub system_version: String,
    /// The client API id.
    #[prost(int32, tag = "6")]
    pub api_id: i32,
    /// The client app version.
    #[prost(string, tag = "7")]
    pub app_version: String,
}

/// Admitted layer evidence sent to `CoreExecService.PublishLayerEvidence`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct PublishLayerEvidenceRequest {
    /// The raw authorization-key id.
    #[prost(bytes = "vec", tag = "1")]
    pub raw_auth_key_id: Vec<u8>,
    /// The MTProto session id.
    #[prost(int64, tag = "2")]
    pub session_id: i64,
    /// The message id.
    #[prost(int64, tag = "3")]
    pub msg_id: i64,
    /// The admission sequence number.
    #[prost(uint64, tag = "4")]
    pub admission_seq: u64,
    /// The safe evidence floor.
    #[prost(uint64, tag = "5")]
    pub safe_floor: u64,
    /// The published layer.
    #[prost(int32, tag = "6")]
    pub layer: i32,
}

/// The answer to `CoreExecService.ResolveNegotiatedSessionLayer`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct ResolveSessionLayerResponse {
    /// The resolved layer.
    #[prost(int32, tag = "1")]
    pub layer: i32,
    /// The evidence message id.
    #[prost(int64, tag = "2")]
    pub msg_id: i64,
    /// Whether evidence was found.
    #[prost(bool, tag = "3")]
    pub found: bool,
    /// A control-plane error.
    #[prost(string, tag = "4")]
    pub error: String,
}

/// The answer to `CoreExecService.ResolveInheritedAuthKeyLayer`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct ResolveInheritedLayerResponse {
    /// The resolved layer.
    #[prost(int32, tag = "1")]
    pub layer: i32,
    /// Whether an inherited layer was found.
    #[prost(bool, tag = "2")]
    pub found: bool,
    /// A control-plane error.
    #[prost(string, tag = "3")]
    pub error: String,
}

/// The answer to `CoreExecService.AdvanceNegotiatedSessionLayer`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct AdvanceSessionLayerResponse {
    /// The resulting current layer.
    #[prost(int32, tag = "1")]
    pub current_layer: i32,
    /// The resulting current evidence message id.
    #[prost(int64, tag = "2")]
    pub current_msg_id: i64,
    /// Whether the shared layer should be published.
    #[prost(bool, tag = "3")]
    pub publish_shared: bool,
    /// A control-plane error.
    #[prost(string, tag = "4")]
    pub error: String,
}

/// The answer to `CoreExecService.DeleteNegotiatedSessionLayer`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct DeleteSessionLayerResponse {
    /// Whether evidence was deleted.
    #[prost(bool, tag = "1")]
    pub deleted: bool,
    /// A control-plane error.
    #[prost(string, tag = "2")]
    pub error: String,
}

/// An initialization connection sent to `CoreExecService.ObserveInitConnection`.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct ObserveInitConnectionRequest {
    /// The authorization-key id.
    #[prost(bytes = "vec", tag = "1")]
    pub auth_key_id: Vec<u8>,
    /// The MTProto session id.
    #[prost(int64, tag = "2")]
    pub session_id: i64,
    /// The observed layer.
    #[prost(int32, tag = "3")]
    pub layer: i32,
    /// The client API id.
    #[prost(int32, tag = "4")]
    pub api_id: i32,
    /// The client device model.
    #[prost(string, tag = "5")]
    pub device_model: String,
    /// The client system version.
    #[prost(string, tag = "6")]
    pub system_version: String,
    /// The client app version.
    #[prost(string, tag = "7")]
    pub app_version: String,
    /// The client system language code.
    #[prost(string, tag = "8")]
    pub system_lang_code: String,
    /// The client language pack.
    #[prost(string, tag = "9")]
    pub lang_pack: String,
    /// The client language code.
    #[prost(string, tag = "10")]
    pub lang_code: String,
    /// The canonical edge-observed peer IP, as in [`DispatchRequest::client_ip`].
    #[prost(string, tag = "11")]
    pub client_ip: String,
}

/// A control-plane result with only an error channel.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct ErrorResponse {
    /// A non-empty diagnosis on failure.
    #[prost(string, tag = "1")]
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::post_response_action::Action;
    use super::{
        AccountAuthorizationTeardownAction, AdmissionMode, CAPABILITIES, CoreExecInfoRequest,
        CoreExecInfoResponse, DispatchRequest, DispatchResponse, ErrorResponse, GetAuthKeyResponse,
        HandshakeError, IdentityHint, Limits, MIN_SUPPORTED_PROTOCOL_VERSION, PROTOCOL_VERSION,
        PROVENANCE, PostResponseAction, PublishLayerEvidenceRequest, RPCError,
        ResolveSessionLayerResponse, SERVICE_NAME, SessionOfflineBatchRequest, SessionOfflineEvent,
        UpdateState, UpdateStateCommitMode, UpdatesDeliveryAction, WireProof, handshake,
        missing_capabilities, protocol_ranges_overlap, provenance, verify_core_info,
        version_is_supported,
    };

    #[test]
    fn dispatch_request_round_trip() {
        let request = DispatchRequest {
            mode: i32::from(AdmissionMode::Layer),
            profile: 232,
            limits: Some(Limits {
                max_wire_bytes: 1 << 20,
                max_vector_elements: 1024,
                max_aggregate_elements: 2048,
                max_depth: 64,
            }),
            request_wire: vec![1, 2, 3, 4],
            proof: Some(WireProof {
                profile: 232,
                method: 0x1234,
                wire_id: 0x1379_4888,
                wire_size: 4,
                wire_digest: "00112233".to_owned(),
                wire_invariant: true,
                effective_profile: Some(super::ProfileMark {
                    profile: 232,
                    present: true,
                }),
                profile_evidence: Some(super::ProfileMark {
                    profile: 232,
                    present: false,
                }),
            }),
            auth_key_id: vec![1, 2, 3, 4, 5, 6, 7, 8],
            session_id: 42,
            msg_id: 99,
            admission_seq: 7,
            profile_evidence_fresh: true,
            identity_hint: Some(IdentityHint {
                business_auth_key_id: vec![9; 8],
                business_auth_key_resolved: true,
                user_id: 100,
                user_id_resolved: true,
                raw_auth_key_expires_at: 1_700_000_000,
                raw_auth_key_expires_at_resolved: true,
                session_updates_ready: false,
            }),
            client_ip: "203.0.113.7".to_owned(),
        };
        let encoded = prost::Message::encode_to_vec(&request);
        let decoded = <DispatchRequest as prost::Message>::decode(encoded.as_slice())
            .expect("decode request");
        assert_eq!(decoded, request);
    }

    #[test]
    fn dispatch_response_round_trip() {
        let response = DispatchResponse {
            method: "messages.sendMessage".to_owned(),
            result_wire: vec![4, 5, 6],
            wire_invariant: true,
            rpc_error: Some(RPCError {
                code: 400,
                message: "BAD_REQUEST".to_owned(),
            }),
            error: String::new(),
            post_response_actions: vec![
                PostResponseAction {
                    action: Some(Action::UpdatesDelivery(UpdatesDeliveryAction {
                        commit_cursor: true,
                        cursor_auth_key_id: vec![1; 8],
                        cursor_user_id: 100,
                        cursor_state: Some(UpdateState {
                            pts: 1,
                            qts: 2,
                            date: 3,
                            seq: 4,
                        }),
                        cursor_mode: i32::from(UpdateStateCommitMode::DeliveredOnly),
                        mark_secret_delivered: true,
                        secret_device_key: 5,
                        secret_event_ids: vec![6, 7],
                        mark_session_ready: true,
                        ready_raw_auth_key_id: vec![2; 8],
                        ready_session_id: 8,
                        ready_user_id: 9,
                        publish_bootstrap: true,
                        bootstrap_user_id: 10,
                        activation_token: 11,
                        bootstrap_auth_key_id: vec![3; 8],
                        bootstrap_session_id: 12,
                        bootstrap_probe_token: 13,
                        bootstrap_raw_auth_key_id: vec![4; 8],
                    })),
                },
                PostResponseAction {
                    action: Some(Action::AccountAuthorizationTeardown(
                        AccountAuthorizationTeardownAction {
                            user_id: 100,
                            auth_key_id: vec![5; 8],
                        },
                    )),
                },
            ],
        };
        let encoded = prost::Message::encode_to_vec(&response);
        let decoded = <DispatchResponse as prost::Message>::decode(encoded.as_slice())
            .expect("decode response");
        assert_eq!(decoded, response);
    }

    #[test]
    fn other_messages_round_trip() {
        let get = GetAuthKeyResponse {
            auth_key: None,
            found: false,
            error: "not found".to_owned(),
        };
        let encoded = prost::Message::encode_to_vec(&get);
        assert_eq!(
            <GetAuthKeyResponse as prost::Message>::decode(encoded.as_slice()).expect("decode"),
            get
        );

        let evidence = PublishLayerEvidenceRequest {
            raw_auth_key_id: vec![7; 8],
            session_id: 1,
            msg_id: 2,
            admission_seq: 3,
            safe_floor: 4,
            layer: 5,
        };
        let encoded = prost::Message::encode_to_vec(&evidence);
        assert_eq!(
            <PublishLayerEvidenceRequest as prost::Message>::decode(encoded.as_slice())
                .expect("decode"),
            evidence
        );

        let resolve = ResolveSessionLayerResponse {
            layer: 232,
            msg_id: 9,
            found: true,
            error: String::new(),
        };
        let encoded = prost::Message::encode_to_vec(&resolve);
        assert_eq!(
            <ResolveSessionLayerResponse as prost::Message>::decode(encoded.as_slice())
                .expect("decode"),
            resolve
        );

        let offline = SessionOfflineBatchRequest {
            events: vec![SessionOfflineEvent {
                raw_auth_key_id: vec![6; 8],
                session_id: 10,
                user_id: 11,
                last_for_user: true,
                disconnected_at: 12,
            }],
        };
        let encoded = prost::Message::encode_to_vec(&offline);
        assert_eq!(
            <SessionOfflineBatchRequest as prost::Message>::decode(encoded.as_slice())
                .expect("decode"),
            offline
        );

        let error = ErrorResponse {
            error: "boom".to_owned(),
        };
        let encoded = prost::Message::encode_to_vec(&error);
        assert_eq!(
            <ErrorResponse as prost::Message>::decode(encoded.as_slice()).expect("decode"),
            error
        );
    }

    #[test]
    fn version_constants_and_ranges() {
        assert_eq!(PROTOCOL_VERSION, 5);
        assert_eq!(MIN_SUPPORTED_PROTOCOL_VERSION, 5);
        assert!(version_is_supported(PROTOCOL_VERSION));
        for version in [0, 1, 4, 6, 99] {
            assert!(!version_is_supported(version), "version {version}");
        }
        assert!(protocol_ranges_overlap(4, 6, 5, 5));
        assert!(!protocol_ranges_overlap(1, 4, 5, 5));
        assert!(!protocol_ranges_overlap(0, 0, 5, 5));
    }

    #[test]
    fn handshake_accepts_version_five_and_rejects_others() {
        let accepted = CoreExecInfoRequest {
            protocol_version: PROTOCOL_VERSION,
            min_supported_protocol_version: MIN_SUPPORTED_PROTOCOL_VERSION,
            capabilities: CAPABILITIES.iter().map(|name| (*name).to_owned()).collect(),
        };
        let response = handshake(&accepted).expect("version 5 is accepted");
        assert_eq!(response.protocol_version, PROTOCOL_VERSION);
        assert_eq!(
            response.min_supported_protocol_version,
            MIN_SUPPORTED_PROTOCOL_VERSION
        );

        for version in [0, 1, 4, 6, 99] {
            let rejected = CoreExecInfoRequest {
                protocol_version: version,
                min_supported_protocol_version: 0,
                capabilities: Vec::new(),
            };
            let error = handshake(&rejected).expect_err("unsupported version is rejected");
            let _: HandshakeError = error;
        }
    }

    #[test]
    fn verify_core_info_checks_version_and_capabilities() {
        let mut response = CoreExecInfoResponse {
            protocol_version: PROTOCOL_VERSION,
            min_supported_protocol_version: MIN_SUPPORTED_PROTOCOL_VERSION,
            capabilities: CAPABILITIES.iter().map(|name| (*name).to_owned()).collect(),
            implementation: "telesrv-coreexec-grpc".to_owned(),
            error: String::new(),
            instance_id: "core-1".to_owned(),
        };
        assert!(verify_core_info(&response).is_ok());
        assert!(missing_capabilities(&response.capabilities).is_empty());

        response.protocol_version = PROTOCOL_VERSION + 1;
        response.min_supported_protocol_version = PROTOCOL_VERSION + 1;
        assert!(verify_core_info(&response).is_err());

        response.protocol_version = PROTOCOL_VERSION;
        response.min_supported_protocol_version = MIN_SUPPORTED_PROTOCOL_VERSION;
        response
            .capabilities
            .retain(|name| name != "auth-key-authority");
        assert_eq!(
            missing_capabilities(&response.capabilities),
            vec!["auth-key-authority"]
        );
        assert!(verify_core_info(&response).is_err());
    }

    #[test]
    fn provenance_is_pinned() {
        assert_eq!(provenance::SOURCE_COMMIT.len(), 40);
        assert!(
            provenance::SOURCE_COMMIT
                .chars()
                .all(|digit| digit.is_ascii_hexdigit())
        );
        assert_eq!(provenance::SOURCE_SHA256.len(), 64);
        assert!(
            provenance::SOURCE_SHA256
                .chars()
                .all(|digit| digit.is_ascii_hexdigit())
        );
        assert_eq!(
            provenance::SOURCE_SHA256,
            "0e177e856541768a22fb986b0647b1e2edb22cb124c7e37c0c11bfc682abdcb4"
        );
        assert_eq!(
            provenance::SOURCE_BLOB,
            "e545a70f702171a3359d7e1231e873ae70d4f995"
        );
        assert!(PROVENANCE.contains(provenance::SOURCE_COMMIT));
        assert!(PROVENANCE.contains(provenance::SOURCE_SHA256));
    }

    #[test]
    fn schema_declares_the_mirrored_contract() {
        let schema = include_str!("../proto/edge.proto");
        assert!(schema.contains("package telesrv.coreexec.v1;"));
        assert!(schema.contains("service CoreExecService {"));
        for method in [
            "GetInfo",
            "DispatchAdmitted",
            "SaveAuthKey",
            "GetAuthKey",
            "DeleteAuthKey",
            "UpdateAuthKeyClientInfo",
            "ResolveNegotiatedSessionLayer",
            "ResolveInheritedAuthKeyLayer",
            "AdvanceNegotiatedSessionLayer",
            "DeleteNegotiatedSessionLayer",
            "PublishLayerEvidence",
            "ObserveInitConnection",
            "ReportSessionOffline",
        ] {
            assert!(schema.contains(method), "schema is missing {method}");
        }
        assert_eq!(SERVICE_NAME, "telesrv.coreexec.v1.CoreExecService");
    }
}
