//! Invocation-wrapper handling, layer binding and dispatch.
//!
//! Owned by task group 9. Implemented in this file.
//!
//! The RPC layer wraps the [`rtd_tlprofile`] admission runtime: transparent
//! invocation wrappers are captured (with a depth bound), the session's layer is
//! bound and corrected, a [`Router`] an embedder can extend produces an
//! identity-bound result, and every failure is mapped to a [`rtd_error::Error`]
//! TL error. The built-in `help.getConfig` and `help.getNearestDc` methods are
//! provided by [`register_builtins`].

use std::collections::HashMap;

use rtd_bin::Writer;
use rtd_error::Error as RtdError;
use rtd_tg::{Config, DcOption, NearestDc, Object};
#[cfg(test)]
use rtd_tg::{
    HelpGetConfig, HelpGetNearestDc, InitConnection, InvokeAfterMsg, InvokeAfterMsgs,
    InvokeWithLayer, Method,
};
use rtd_tlprofile::semantic::{METHOD_HELP_GET_CONFIG, METHOD_HELP_GET_NEAREST_DC};
use rtd_tlprofile::{
    Admission, AdmitError, CallIdentity, ClientInfo, FrozenObject, Handler, OrderingKey, Profile,
    ProjectionError, SemanticId, WrapperKind, resolve_profile,
};

use crate::ratelimit::{MethodClass, RateLimiter};

/// TL error code for a method the router does not handle.
pub const CODE_NOT_FOUND: i32 = 404;
/// TL error code for a malformed or rejected request.
pub const CODE_BAD_REQUEST: i32 = 400;
/// TL error code for an internal failure.
pub const CODE_INTERNAL: i32 = 500;
/// TL error code telling a client to wait before retrying.
pub const CODE_FLOOD_WAIT: i32 = 420;

/// TL error message for an unknown method.
pub const ERR_METHOD_NOT_FOUND: &str = "METHOD_NOT_FOUND";
/// TL error message for a result whose identity differs from the request.
pub const ERR_RESULT_IDENTITY: &str = "RESULT_IDENTITY_MISMATCH";
/// TL error message for call metadata that differs from the request.
pub const ERR_CALL_IDENTITY: &str = "CALL_IDENTITY_MISMATCH";
/// TL error message for a request that nests too many wrappers.
pub const ERR_WRAPPER_DEPTH: &str = "WRAPPER_DEPTH_EXCEEDED";

/// The maximum number of transparent wrappers accepted around a terminal.
pub const MAX_WRAPPER_DEPTH: usize = 8;

/// Returns the TL error for an unknown method.
#[must_use]
pub fn unknown_method_error() -> RtdError {
    RtdError::new(CODE_NOT_FOUND, ERR_METHOD_NOT_FOUND)
}

/// Returns the TL error for a result whose identity differs from the request.
#[must_use]
pub fn result_identity_error() -> RtdError {
    RtdError::new(CODE_INTERNAL, ERR_RESULT_IDENTITY)
}

/// Returns the `FLOOD_WAIT_<seconds>` TL error.
#[must_use]
pub fn flood_wait_error(seconds: u64) -> RtdError {
    RtdError::new(CODE_FLOOD_WAIT, format!("FLOOD_WAIT_{seconds}"))
}

/// Returns the TL error for call metadata that differs from the request.
#[must_use]
pub fn call_identity_error() -> RtdError {
    RtdError::new(CODE_BAD_REQUEST, ERR_CALL_IDENTITY)
}

/// Returns the TL error for a request selecting an unsupported layer.
#[must_use]
pub fn unsupported_layer_error(layer: i32) -> RtdError {
    RtdError::new(CODE_BAD_REQUEST, format!("LAYER_{layer}_UNSUPPORTED"))
}

/// Returns the TL error for a handler that rejects a call.
#[must_use]
pub fn handler_error(message: impl Into<String>) -> RtdError {
    RtdError::new(CODE_BAD_REQUEST, message)
}

/// Maps a tlprofile admission error to a TL error.
#[must_use]
pub fn map_admit_error(error: &AdmitError) -> RtdError {
    match error {
        AdmitError::Decode(_) => RtdError::new(CODE_BAD_REQUEST, "REQUEST_DECODE_FAILED"),
        AdmitError::UnsupportedLayer(_) => RtdError::new(CODE_BAD_REQUEST, "LAYER_INVALID"),
        AdmitError::Conflict(..) => RtdError::new(CODE_BAD_REQUEST, "LAYER_CONFLICT"),
        AdmitError::RequiresProfile(_) => RtdError::new(CODE_BAD_REQUEST, "LAYER_REQUIRED"),
        AdmitError::UnsupportedTerminal(_) => unknown_method_error(),
        AdmitError::UnsupportedRoute(_) => RtdError::new(CODE_INTERNAL, "ROUTE_UNAVAILABLE"),
        AdmitError::Limit { .. } => RtdError::new(CODE_BAD_REQUEST, "REQUEST_LIMIT_EXCEEDED"),
        AdmitError::Hook(_) => RtdError::new(CODE_BAD_REQUEST, "LAYER_CONVERSION_FAILED"),
    }
}

/// Maps a tlprofile projection error to a TL error.
#[must_use]
pub fn map_projection_error(_error: &ProjectionError) -> RtdError {
    RtdError::new(CODE_INTERNAL, "PROJECTION_FAILED")
}

/// A canonical handler result bound to the admitted call identity.
///
/// [`Router`] implementations return this so the identity of the request that
/// produced the result can be checked before it is encoded for the session.
#[derive(Debug, Clone)]
pub struct RoutedResult {
    identity: CallIdentity,
    value: FrozenObject,
}

impl RoutedResult {
    /// Freezes `value` and binds it to `identity`.
    ///
    /// # Errors
    ///
    /// Returns a TL error when the value has no canonical route.
    pub fn bind(identity: CallIdentity, value: Object) -> Result<Self, RtdError> {
        let value = FrozenObject::freeze(value).map_err(|error| map_projection_error(&error))?;
        Ok(Self { identity, value })
    }

    /// Returns the bound admitted call identity.
    #[must_use]
    pub fn identity(&self) -> &CallIdentity {
        &self.identity
    }

    /// Returns the frozen canonical value.
    #[must_use]
    pub fn value(&self) -> &FrozenObject {
        &self.value
    }

    /// Encodes the result for an exact profile, failing closed.
    ///
    /// # Errors
    ///
    /// As [`FrozenObject::encode`].
    pub fn encode(&self, profile: Profile, out: &mut Writer) -> Result<(), ProjectionError> {
        self.value.encode(profile, out)
    }
}

/// The wrapper metadata captured from an admitted request.
#[derive(Debug, Clone, Default)]
pub struct WrapperTrace {
    /// Client and connection info from the outermost `initConnection`, if any.
    pub client: Option<ClientInfo>,
    /// The accumulated invoke-after ordering key.
    pub ordering: OrderingKey,
    /// The consumed wrapper kinds, outermost first.
    pub kinds: Vec<WrapperKind>,
}

/// Captures the wrapper metadata of an admitted request, enforcing the depth bound.
///
/// The wrappers themselves are unwrapped by [`rtd_tlprofile`] admission; this
/// interprets them for the session (initialization info and ordering) and fails
/// closed when the chain is deeper than [`MAX_WRAPPER_DEPTH`].
///
/// # Errors
///
/// Returns a TL error when the request carries more than [`MAX_WRAPPER_DEPTH`]
/// wrappers.
pub fn capture_wrappers(admission: &Admission) -> Result<WrapperTrace, RtdError> {
    let wrappers = admission.wrappers();
    if wrappers.len() > MAX_WRAPPER_DEPTH {
        return Err(RtdError::new(CODE_BAD_REQUEST, ERR_WRAPPER_DEPTH));
    }
    let mut trace = WrapperTrace {
        ordering: admission.ordering().clone(),
        ..WrapperTrace::default()
    };
    for wrapper in wrappers {
        trace.kinds.push(wrapper.kind());
        if wrapper.kind() == WrapperKind::InitConnection {
            trace.client = wrapper.client().cloned();
        }
    }
    Ok(trace)
}

/// A configured datacenter advertised by `help.getConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataCenter {
    /// The datacenter identifier.
    pub id: i32,
    /// The advertised IP address.
    pub ip_address: String,
    /// The advertised port.
    pub port: i32,
}

impl DataCenter {
    /// Creates a datacenter entry.
    #[must_use]
    pub fn new(id: i32, ip_address: impl Into<String>, port: i32) -> Self {
        Self {
            id,
            ip_address: ip_address.into(),
            port,
        }
    }
}

/// The minimal server configuration read by the built-in methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// The datacenter serving requests.
    pub dc: i32,
    /// The configured datacenter set.
    pub datacenters: Vec<DataCenter>,
}

impl ServerConfig {
    /// Creates a configuration for a serving datacenter and a datacenter set.
    #[must_use]
    pub fn new(dc: i32, datacenters: Vec<DataCenter>) -> Self {
        Self { dc, datacenters }
    }
}

/// The set of RPC methods an embedder supplies.
///
/// An embedder extends the router by registering a handler for a [`SemanticId`];
/// the layer binding, result-identity binding and TL error mapping are performed
/// by [`dispatch`].
pub trait Router: Send + Sync {
    /// Registers `handler` for `semantic`, replacing any previous handler.
    fn register(&mut self, semantic: SemanticId, handler: Box<dyn Handler + Send + Sync>);

    /// Routes an admitted call to a canonical result bound to its call identity.
    ///
    /// # Errors
    ///
    /// Returns a TL error when no handler is registered for the method or the
    /// handler rejects the call.
    fn route(&self, admission: &Admission) -> Result<RoutedResult, RtdError>;
}

/// The default [`Router`], backed by a semantic handler map.
pub struct HandlerRouter {
    handlers: HashMap<u64, Box<dyn Handler + Send + Sync>>,
    rate_limit: Option<RateLimiter>,
}

impl HandlerRouter {
    /// Creates an empty router.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            rate_limit: None,
        }
    }

    /// Attaches a per-method rate limiter consulted before dispatch.
    #[must_use]
    pub fn with_rate_limit(mut self, limiter: RateLimiter) -> Self {
        self.rate_limit = Some(limiter);
        self
    }

    /// Creates a router serving the built-in methods configured by `config`.
    #[must_use]
    pub fn with_builtins(config: &ServerConfig) -> Self {
        let mut router = Self::new();
        register_builtins(&mut router, config);
        router
    }
}

impl Default for HandlerRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl Router for HandlerRouter {
    fn register(&mut self, semantic: SemanticId, handler: Box<dyn Handler + Send + Sync>) {
        self.handlers.insert(semantic.get(), handler);
    }

    fn route(&self, admission: &Admission) -> Result<RoutedResult, RtdError> {
        if let Some(limiter) = &self.rate_limit {
            // The identity is deployment-global for now; per-session identity is a
            // later refinement that the actor can supply.
            let class = MethodClass::of(admission.semantic);
            limiter
                .check(class, 0)
                .map_err(|wait| flood_wait_error(wait.as_secs().max(1)))?;
        }
        let handler = self
            .handlers
            .get(&admission.semantic.get())
            .ok_or_else(unknown_method_error)?;
        let value = handler.handle(admission).map_err(handler_error)?;
        RoutedResult::bind(admission.call_identity().clone(), value)
    }
}

/// Registers the built-in `help.getConfig` and `help.getNearestDc` handlers.
pub fn register_builtins<R: Router + ?Sized>(router: &mut R, config: &ServerConfig) {
    let this_dc = config.dc;
    let config = config.clone();
    router.register(
        METHOD_HELP_GET_CONFIG,
        Box::new(move |_admission: &Admission| -> Result<Object, String> {
            Ok(build_config(&config))
        }),
    );
    router.register(
        METHOD_HELP_GET_NEAREST_DC,
        Box::new(move |_admission: &Admission| -> Result<Object, String> {
            Ok(build_nearest_dc(this_dc))
        }),
    );
}

/// Dispatches an admitted call through `router`, binding the result identity.
///
/// # Errors
///
/// Returns a TL error when the router rejects the call or returns a result that
/// is not bound to the admitted call identity.
pub fn dispatch<R: Router + ?Sized>(
    router: &R,
    admission: &Admission,
) -> Result<RoutedResult, RtdError> {
    let result = router.route(admission)?;
    if result.identity() != admission.call_identity() {
        return Err(result_identity_error());
    }
    Ok(result)
}

/// How a session's layer was resolved for one admitted request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerSelection {
    /// The request explicitly selected the layer (`invokeWithLayer`).
    Explicit(Profile),
    /// The session's previously bound layer was reused.
    Inherited(Profile),
    /// No layer was bound, so the configured default applied.
    Default(Profile),
}

impl LayerSelection {
    /// Returns the resolved profile.
    #[must_use]
    pub fn profile(self) -> Profile {
        match self {
            LayerSelection::Explicit(profile)
            | LayerSelection::Inherited(profile)
            | LayerSelection::Default(profile) => profile,
        }
    }
}

/// A per-session layer binding.
///
/// Binding prefers an explicit `invokeWithLayer` selection, then inherits the
/// session's previously bound layer, then the configured default. Explicit
/// selections are applied in client message-id order and fail closed for an
/// unsupported layer.
#[derive(Debug, Clone)]
pub struct LayerBinding {
    default: Profile,
    bound: Option<Profile>,
    applied: Option<i64>,
}

impl LayerBinding {
    /// Creates a binding inheriting `default` until a request selects a layer.
    #[must_use]
    pub fn new(default: Profile) -> Self {
        Self {
            default,
            bound: None,
            applied: None,
        }
    }

    /// Returns the currently bound profile, if any.
    #[must_use]
    pub fn bound(&self) -> Option<Profile> {
        self.bound
    }

    /// Resolves the layer for an admitted request, correcting the binding.
    ///
    /// # Errors
    ///
    /// Fails closed when the request explicitly selects an unsupported layer.
    pub fn resolve(&mut self, admission: &Admission) -> Result<LayerSelection, RtdError> {
        match admission.explicit_profile {
            Some(explicit) => self.correct(explicit.layer(), admission.ordering()),
            None => Ok(self.current()),
        }
    }

    /// Applies an explicit layer selection, ordered by its invoke-after key.
    ///
    /// A selection whose ordering key is not newer than the last applied
    /// correction is ignored, so an out-of-order request cannot regress the layer.
    ///
    /// # Errors
    ///
    /// Fails closed when `layer` is unsupported.
    pub fn correct(
        &mut self,
        layer: i32,
        ordering: &OrderingKey,
    ) -> Result<LayerSelection, RtdError> {
        let profile = resolve_profile(layer).ok_or_else(|| unsupported_layer_error(layer))?;
        let position = ordering.as_slice().last().copied();
        if let (Some(position), Some(applied)) = (position, self.applied)
            && position <= applied
        {
            return Ok(self.current());
        }
        if let Some(position) = position {
            self.applied = Some(position);
        }
        self.bound = Some(profile);
        Ok(LayerSelection::Explicit(profile))
    }

    /// Encodes a dispatched result for the session's profile, failing closed.
    ///
    /// # Errors
    ///
    /// Returns a TL error when the result is not representable in the profile.
    pub fn encode(&self, result: &RoutedResult) -> Result<Vec<u8>, RtdError> {
        let profile = self.bound.unwrap_or(self.default);
        let mut writer = Writer::new();
        result
            .encode(profile, &mut writer)
            .map_err(|error| map_projection_error(&error))?;
        Ok(writer.into_vec())
    }

    /// Returns the selection implied by the current binding.
    fn current(&self) -> LayerSelection {
        match self.bound {
            Some(profile) => LayerSelection::Inherited(profile),
            None => LayerSelection::Default(self.default),
        }
    }
}

/// Builds the `help.getConfig` result for `config`.
fn build_config(config: &ServerConfig) -> Object {
    let dc_options = config
        .datacenters
        .iter()
        .map(|dc| DcOption {
            flags: 0,
            ipv6: false,
            media_only: false,
            tcpo_only: false,
            cdn: false,
            static_: false,
            this_port_only: false,
            id: dc.id,
            ip_address: dc.ip_address.clone(),
            port: dc.port,
            secret: None,
        })
        .collect();
    Object::Config(Box::new(Config {
        flags: 0,
        default_p2p_contacts: false,
        preload_featured_stickers: false,
        revoke_pm_inbox: false,
        blocked_mode: false,
        force_try_ipv6: false,
        date: 0,
        expires: 0,
        test_mode: false,
        this_dc: config.dc,
        dc_options,
        dc_txt_domain_name: String::new(),
        chat_size_max: 200,
        megagroup_size_max: 200,
        forwarded_count_max: 100,
        online_update_period_ms: 0,
        offline_blur_timeout_ms: 0,
        offline_idle_timeout_ms: 0,
        online_cloud_timeout_ms: 0,
        notify_cloud_delay_ms: 0,
        notify_default_delay_ms: 0,
        push_chat_period_ms: 0,
        push_chat_limit: 0,
        edit_time_limit: 0,
        revoke_time_limit: 0,
        revoke_pm_time_limit: 0,
        rating_e_decay: 0,
        stickers_recent_limit: 0,
        channels_read_media_period: 0,
        tmp_sessions: None,
        call_receive_timeout_ms: 0,
        call_ring_timeout_ms: 0,
        call_connect_timeout_ms: 0,
        call_packet_timeout_ms: 0,
        me_url_prefix: String::new(),
        autoupdate_url_prefix: None,
        gif_search_username: None,
        venue_search_username: None,
        img_search_username: None,
        static_maps_provider: None,
        caption_length_max: 0,
        message_length_max: 0,
        webfile_dc_id: 0,
        suggested_lang_code: None,
        lang_pack_version: None,
        base_lang_pack_version: None,
        reactions_default: None,
        autologin_token: None,
    }))
}

/// Builds the `help.getNearestDc` result for the serving datacenter.
fn build_nearest_dc(this_dc: i32) -> Object {
    Object::NearestDc(Box::new(NearestDc {
        country: String::new(),
        this_dc,
        nearest_dc: this_dc,
    }))
}

/// Encodes a TL method, including its constructor identifier.
#[cfg(test)]
fn encode_method(method: &Method) -> Vec<u8> {
    let mut writer = Writer::new();
    method.encode(&mut writer);
    writer.into_vec()
}

/// Encodes a nested chain of `invokeWithLayer` wrappers around `help.getConfig`.
#[cfg(test)]
fn nested_layer_wrappers(depth: usize) -> Vec<u8> {
    let mut method = Method::HelpGetConfig(Box::new(HelpGetConfig));
    for _ in 0..depth {
        method = Method::InvokeWithLayer(Box::new(InvokeWithLayer {
            layer: 232,
            query: Box::new(method),
        }));
    }
    encode_method(&method)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtd_tlprofile::{admit_default, decode_object};

    fn canonical() -> Profile {
        Profile::new(232)
    }

    fn admit_config() -> Admission {
        let method = Method::HelpGetConfig(Box::new(HelpGetConfig));
        admit_default(canonical(), &encode_method(&method)).expect("admit help.getConfig")
    }

    #[test]
    fn wrapper_depth_limit_rejected() {
        let bytes = nested_layer_wrappers(MAX_WRAPPER_DEPTH + 1);
        let admission = admit_default(canonical(), &bytes).expect("decode nested wrappers");
        assert_eq!(admission.wrappers().len(), MAX_WRAPPER_DEPTH + 1);
        let error = capture_wrappers(&admission).expect_err("depth bound rejects");
        assert_eq!(error.code(), CODE_BAD_REQUEST);
        assert_eq!(error.message(), ERR_WRAPPER_DEPTH);

        let bytes = nested_layer_wrappers(MAX_WRAPPER_DEPTH);
        let admission = admit_default(canonical(), &bytes).expect("decode nested wrappers");
        assert!(capture_wrappers(&admission).is_ok());
    }

    #[test]
    fn init_connection_captured() {
        let method = Method::InitConnection(Box::new(InitConnection {
            flags: 0,
            api_id: 12_345,
            device_model: "device".to_owned(),
            system_version: "1.0".to_owned(),
            app_version: "2.0".to_owned(),
            system_lang_code: "en".to_owned(),
            lang_pack: String::new(),
            lang_code: "en".to_owned(),
            proxy: None,
            params: None,
            query: Box::new(Method::HelpGetConfig(Box::new(HelpGetConfig))),
        }));
        let admission = admit_default(canonical(), &encode_method(&method)).expect("admit");
        let trace = capture_wrappers(&admission).expect("capture");
        let client = trace.client.expect("initConnection captured");
        assert_eq!(client.api_id, 12_345);
        assert_eq!(client.device_model, "device");
        assert_eq!(client.app_version, "2.0");
        assert_eq!(trace.kinds, vec![WrapperKind::InitConnection]);
    }

    #[test]
    fn invoke_after_ordering() {
        let method = Method::InvokeAfterMsg(Box::new(InvokeAfterMsg {
            msg_id: 7,
            query: Box::new(Method::HelpGetConfig(Box::new(HelpGetConfig))),
        }));
        let admission = admit_default(canonical(), &encode_method(&method)).expect("admit");
        let trace = capture_wrappers(&admission).expect("capture");
        assert_eq!(trace.ordering.as_slice(), &[7]);
        assert_eq!(trace.kinds, vec![WrapperKind::InvokeAfterMsg]);

        let method = Method::InvokeAfterMsgs(Box::new(InvokeAfterMsgs {
            msg_ids: vec![1, 2],
            query: Box::new(Method::HelpGetConfig(Box::new(HelpGetConfig))),
        }));
        let admission = admit_default(canonical(), &encode_method(&method)).expect("admit");
        let trace = capture_wrappers(&admission).expect("capture");
        assert_eq!(trace.ordering.as_slice(), &[1, 2]);
    }

    #[test]
    fn inherited_default_layer() {
        let mut binding = LayerBinding::new(canonical());
        assert_eq!(
            binding.resolve(&admit_config()).expect("resolve"),
            LayerSelection::Default(canonical())
        );

        let explicit = Method::InvokeWithLayer(Box::new(InvokeWithLayer {
            layer: 232,
            query: Box::new(Method::HelpGetConfig(Box::new(HelpGetConfig))),
        }));
        let admission = admit_default(canonical(), &encode_method(&explicit)).expect("admit");
        assert_eq!(
            binding.resolve(&admission).expect("resolve"),
            LayerSelection::Explicit(canonical())
        );
        assert_eq!(binding.bound(), Some(canonical()));

        assert_eq!(
            binding.resolve(&admit_config()).expect("resolve"),
            LayerSelection::Inherited(canonical())
        );
    }

    #[test]
    fn ordered_correction() {
        let mut binding = LayerBinding::new(canonical());
        let newer_first = Method::InvokeWithLayer(Box::new(InvokeWithLayer {
            layer: 232,
            query: Box::new(Method::InvokeAfterMsg(Box::new(InvokeAfterMsg {
                msg_id: 10,
                query: Box::new(Method::HelpGetConfig(Box::new(HelpGetConfig))),
            }))),
        }));
        let older = Method::InvokeAfterMsg(Box::new(InvokeAfterMsg {
            msg_id: 5,
            query: Box::new(Method::InvokeWithLayer(Box::new(InvokeWithLayer {
                layer: 232,
                query: Box::new(Method::HelpGetConfig(Box::new(HelpGetConfig))),
            }))),
        }));

        let admission = admit_default(canonical(), &encode_method(&newer_first)).expect("admit");
        assert_eq!(
            binding.resolve(&admission).expect("resolve"),
            LayerSelection::Explicit(canonical())
        );

        let admission = admit_default(canonical(), &encode_method(&older)).expect("admit");
        assert_eq!(admission.ordering().as_slice(), &[5]);
        // The older correction is superseded and cannot regress the binding.
        assert_eq!(
            binding.resolve(&admission).expect("resolve"),
            LayerSelection::Inherited(canonical())
        );
    }

    #[test]
    fn fail_closed_unsupported_layer() {
        let mut binding = LayerBinding::new(canonical());
        assert!(binding.correct(224, &OrderingKey::default()).is_err());

        let bad = Method::InvokeWithLayer(Box::new(InvokeWithLayer {
            layer: 224,
            query: Box::new(Method::HelpGetConfig(Box::new(HelpGetConfig))),
        }));
        let error = admit_default(canonical(), &encode_method(&bad)).expect_err("admit fails");
        assert_eq!(map_admit_error(&error).code(), CODE_BAD_REQUEST);
    }

    struct TestRouter {
        handlers: HashMap<u64, Box<dyn Handler + Send + Sync>>,
    }

    impl Router for TestRouter {
        fn register(&mut self, semantic: SemanticId, handler: Box<dyn Handler + Send + Sync>) {
            self.handlers.insert(semantic.get(), handler);
        }

        fn route(&self, admission: &Admission) -> Result<RoutedResult, RtdError> {
            let handler = self
                .handlers
                .get(&admission.semantic.get())
                .ok_or_else(unknown_method_error)?;
            let value = handler.handle(admission).map_err(handler_error)?;
            RoutedResult::bind(admission.call_identity().clone(), value)
        }
    }

    #[test]
    fn custom_router_handles_method() {
        let mut router = TestRouter {
            handlers: HashMap::new(),
        };
        router.register(
            METHOD_HELP_GET_CONFIG,
            Box::new(|_admission: &Admission| -> Result<Object, String> {
                Ok(Object::NearestDc(Box::new(NearestDc {
                    country: "ZZ".to_owned(),
                    this_dc: 9,
                    nearest_dc: 9,
                })))
            }),
        );
        let result = dispatch(&router, &admit_config()).expect("dispatch");
        assert_eq!(result.value().type_id(), NearestDc::TYPE_ID);
    }

    struct WrongRouter {
        other: Admission,
    }

    impl Router for WrongRouter {
        fn register(&mut self, _semantic: SemanticId, _handler: Box<dyn Handler + Send + Sync>) {}

        fn route(&self, _admission: &Admission) -> Result<RoutedResult, RtdError> {
            RoutedResult::bind(self.other.call_identity().clone(), build_nearest_dc(1))
        }
    }

    #[test]
    fn mismatched_result_rejected() {
        let other_method = Method::HelpGetNearestDc(Box::new(HelpGetNearestDc));
        let other = admit_default(canonical(), &encode_method(&other_method)).expect("admit");
        let router = WrongRouter { other };

        let error = dispatch(&router, &admit_config()).expect_err("mismatch rejected");
        assert_eq!(error.message(), ERR_RESULT_IDENTITY);
    }

    #[test]
    fn unknown_method_fallback() {
        let router = HandlerRouter::new();
        let error = dispatch(&router, &admit_config()).expect_err("unknown method");
        assert_eq!(error.code(), CODE_NOT_FOUND);
        assert_eq!(error.message(), ERR_METHOD_NOT_FOUND);
    }

    #[test]
    fn builtin_get_config() {
        let config = ServerConfig::new(
            2,
            vec![
                DataCenter::new(2, "127.0.0.1", 443),
                DataCenter::new(3, "127.0.0.2", 443),
            ],
        );
        let router = HandlerRouter::with_builtins(&config);
        let result = dispatch(&router, &admit_config()).expect("dispatch");

        let Object::Config(returned) = result.value().canonical() else {
            panic!("help.getConfig did not return a Config");
        };
        assert_eq!(returned.this_dc, 2);
        assert_eq!(returned.dc_options.len(), 2);
        assert_eq!(returned.dc_options[0].id, 2);
        assert_eq!(returned.dc_options[1].id, 3);

        // Per-profile result encoding round-trips.
        let binding = LayerBinding::new(canonical());
        let bytes = binding.encode(&result).expect("encode");
        let decoded = decode_object(canonical(), &bytes, rtd_tlprofile::Limits::default())
            .expect("decode encoded result");
        assert_eq!(decoded.type_id(), Config::TYPE_ID);
    }

    #[test]
    fn builtin_get_nearest_dc() {
        let config = ServerConfig::new(5, Vec::new());
        let router = HandlerRouter::with_builtins(&config);
        let method = Method::HelpGetNearestDc(Box::new(HelpGetNearestDc));
        let admission = admit_default(canonical(), &encode_method(&method)).expect("admit");
        let result = dispatch(&router, &admission).expect("dispatch");

        let Object::NearestDc(returned) = result.value().canonical() else {
            panic!("help.getNearestDc did not return a NearestDc");
        };
        assert_eq!(returned.this_dc, 5);
        assert_eq!(returned.nearest_dc, 5);
    }
}
