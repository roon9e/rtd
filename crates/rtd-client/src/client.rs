//! A synchronous MTProto 2.0 client over a [`Transport`].

use std::sync::Arc;

use rtd_bin::{Reader, Writer};
use rtd_crypto::srp::{Input as SrpInput, hash as srp_hash};
use rtd_crypto::{AuthKey, Key, SystemRandom};
use rtd_exchange::{ClientConfig, Stream as ExchangeStream, client_exchange};
use rtd_mt::{
    BadMsgNotification, DecodeLimits as ServiceDecodeLimits, Decoder as ServiceDecoder, FutureSalt,
    FutureSalts, Get_future_salts, Method as ServiceMethod, MsgsAck, Object as ServiceObject, Ping,
    Pong, RpcError,
};
use rtd_proto::{
    GZIP_TYPE_ID, Gzip, MESSAGE_CONTAINER_TYPE_ID, MessageContainer, MessageSeq,
    RPC_RESULT_TYPE_ID, RpcResult,
};
use rtd_tg::{
    AccountGetPassword, AuthAcceptLoginToken, AuthAuthorization, AuthCheckPassword,
    AuthCheckRecoveryPassword, AuthExportLoginToken, AuthImportBotAuthorization,
    AuthImportLoginToken, AuthLoginToken, AuthPasswordRecovery, AuthRecoverPassword,
    AuthRequestPasswordRecovery, AuthSendCode, AuthSentCode, AuthSignIn, CodeSettings,
    DecodeLimits, Decoder, HelpGetConfig, InitConnection, InputCheckPasswordSRP, InvokeWithLayer,
    LAYER, Method, Object, PasswordKdfAlgo, Update, Updates,
};
use rtd_util::Clock;

use crate::auth::PasswordProvider;
use crate::error::{Error, Result};
use crate::middleware::{Middleware, run_chain};
use crate::qr::LoginToken;
use crate::session::{ClientSession, SessionData};
use crate::transport::Transport;

/// `bad_server_salt` constructor id.
const BAD_SERVER_SALT_ID: u32 = 0xedab_447b;
/// `bad_msg_notification` constructor id.
const BAD_MSG_NOTIFICATION_ID: u32 = 0xa7ef_f811;
/// `future_salts` constructor id.
const FUTURE_SALTS_ID: u32 = 0xae50_0895;
/// Maximum receive rounds while awaiting one result.
const MAX_ROUNDS: usize = 64;
/// Maximum send attempts (initial plus retry).
const MAX_ATTEMPTS: usize = 8;

/// Parameters for the first `initConnection` call.
#[derive(Debug, Clone)]
pub struct InitParams {
    /// The application id.
    pub api_id: i32,
    /// The device model.
    pub device_model: String,
    /// The operating system version.
    pub system_version: String,
    /// The client application version.
    pub app_version: String,
    /// The system language code.
    pub system_lang_code: String,
    /// The language pack name.
    pub lang_pack: String,
    /// The language code.
    pub lang_code: String,
}

/// A synchronous MTProto 2.0 client over a [`Transport`].
pub struct Client<T: Transport> {
    transport: T,
    session: ClientSession,
    random: SystemRandom,
    initialized: bool,
    handler: Option<Box<dyn FnMut(Updates) + Send>>,
    /// The layer every call is wrapped in.
    layer: i32,
    /// Invoker middleware applied to every call, outermost first.
    middlewares: Vec<Box<dyn Middleware + Send>>,
}

/// One inbound message, reduced to what the caller cares about.
enum Reply {
    /// The awaited result arrived.
    Result(Object),
    /// The server reported a stale salt; resend with the refreshed one.
    BadSalt,
    /// A message that did not answer this request.
    Ignored,
}

impl<T: Transport> Client<T> {
    /// Connects over an established transport: completes the requester
    /// authorization-key exchange and returns a client bound to the result.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Exchange`] when the key exchange fails, or a transport
    /// error.
    pub fn connect(
        mut transport: T,
        config: &ClientConfig,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self> {
        let result = client_exchange(
            &mut ExchangeAdapter(&mut transport),
            config,
            clock.as_ref(),
            &mut SystemRandom,
        )?;
        Ok(Self::new(
            transport,
            result.auth_key,
            result.session_id,
            result.server_salt,
            clock,
        ))
    }

    /// Creates a client over an established session.
    #[must_use]
    pub fn new(
        transport: T,
        auth_key: AuthKey,
        session_id: i64,
        salt: i64,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            transport,
            session: ClientSession::new(auth_key, session_id, salt, clock),
            random: SystemRandom,
            initialized: false,
            handler: None,
            layer: LAYER,
            middlewares: Vec::new(),
        }
    }

    /// Appends a middleware to the invocation chain.
    ///
    /// Middlewares run outermost-first in the order they are added.
    pub fn with_middleware(mut self, middleware: impl Middleware + 'static) -> Self {
        self.middlewares.push(Box::new(middleware));
        self
    }

    /// Sets the layer every call is wrapped in (`invokeWithLayer`).
    ///
    /// Defaults to the compiled canonical layer; a server that speaks another
    /// layer (for example a gramsrv v2 core at layer 229) can be targeted before
    /// the first call.
    pub fn set_layer(&mut self, layer: i32) {
        self.layer = layer;
    }

    /// Returns the current server salt.
    #[must_use]
    pub fn salt(&self) -> i64 {
        self.session.salt()
    }

    /// Snapshots the session for persistence; restore it with
    /// [`Client::from_session`].
    #[must_use]
    pub fn session_data(&self) -> SessionData {
        self.session.snapshot()
    }

    /// Builds a client over a fresh transport from a saved session, without
    /// repeating the authorization-key exchange.
    #[must_use]
    pub fn from_session(
        transport: T,
        data: &SessionData,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        let auth_key = Key::from_array(data.auth_key).with_id();
        Self::new(transport, auth_key, data.session_id, data.salt, clock)
    }

    /// Sets the handler invoked for server updates received while a call awaits
    /// its reply.
    ///
    /// An `Updates` envelope is passed as decoded; a bare `Update` is wrapped in
    /// [`Updates::UpdateShort`].
    pub fn set_update_handler(&mut self, handler: impl FnMut(Updates) + Send + 'static) {
        self.handler = Some(Box::new(handler));
    }

    /// Establishes the session with `invokeWithLayer`/`initConnection` at the
    /// canonical layer, invoking `help.getConfig`, and returns the inner result.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error.
    pub fn initialize(&mut self, params: InitParams) -> Result<Object> {
        let init = Method::InitConnection(Box::new(InitConnection {
            flags: 0,
            api_id: params.api_id,
            device_model: params.device_model,
            system_version: params.system_version,
            app_version: params.app_version,
            system_lang_code: params.system_lang_code,
            lang_pack: params.lang_pack,
            lang_code: params.lang_code,
            proxy: None,
            params: None,
            query: Box::new(Method::HelpGetConfig(Box::new(HelpGetConfig))),
        }));
        let body = self.wrap_layer(init);
        let result = self.call(body)?;
        self.initialized = true;
        Ok(result)
    }

    /// Invokes one canonical-layer method and returns its result.
    ///
    /// Every call is wrapped in `invokeWithLayer` at the canonical layer, like
    /// `td`; [`Client::initialize`] must run first for a fresh session.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error.
    pub fn invoke(&mut self, query: Method) -> Result<Object> {
        // Move the chain out so the inner closure can borrow `self` mutably.
        let middlewares = core::mem::take(&mut self.middlewares);
        let result = run_chain(&middlewares, query, &mut |method| {
            let body = self.wrap_layer(method);
            self.call(body)
        });
        self.middlewares = middlewares;
        result
    }

    /// Sends a login code to `phone_number` (`auth.sendCode`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or
    /// [`Error::Unexpected`] when the server answers with a different constructor.
    pub fn send_code(
        &mut self,
        phone_number: &str,
        api_id: i32,
        api_hash: &str,
        settings: CodeSettings,
    ) -> Result<AuthSentCode> {
        let query = Method::AuthSendCode(Box::new(AuthSendCode {
            phone_number: phone_number.to_owned(),
            api_id,
            api_hash: api_hash.to_owned(),
            settings: Box::new(settings),
        }));
        match self.invoke(query)? {
            Object::AuthSentCode(code) => Ok(*code),
            other => Err(unexpected("auth.sendCode", &other)),
        }
    }

    /// Signs in with a code from [`Client::send_code`] (`auth.signIn`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or
    /// [`Error::Unexpected`] when the server answers with a different constructor.
    pub fn sign_in(
        &mut self,
        phone_number: &str,
        phone_code_hash: &str,
        phone_code: Option<&str>,
    ) -> Result<AuthAuthorization> {
        let query = Method::AuthSignIn(Box::new(AuthSignIn {
            flags: if phone_code.is_some() { 1 } else { 0 },
            phone_number: phone_number.to_owned(),
            phone_code_hash: phone_code_hash.to_owned(),
            phone_code: phone_code.map(str::to_owned),
            email_verification: None,
        }));
        match self.invoke(query)? {
            Object::AuthAuthorization(authorization) => Ok(*authorization),
            other => Err(unexpected("auth.signIn", &other)),
        }
    }

    /// Completes two-factor sign-in for `password` (`account.getPassword` then
    /// `auth.checkPassword`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server does not present an SRP challenge.
    pub fn check_password(&mut self, password: &[u8]) -> Result<AuthAuthorization> {
        let state = match self.invoke(Method::AccountGetPassword(Box::new(AccountGetPassword)))? {
            Object::AccountPassword(state) => *state,
            other => return Err(unexpected("account.getPassword", &other)),
        };
        let (Some(algo), Some(srp_b), Some(srp_id)) =
            (state.current_algo, state.srp_b, state.srp_id)
        else {
            return Err(Error::Unexpected(
                "account.getPassword did not return an SRP challenge".to_owned(),
            ));
        };
        let input = match *algo {
            PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow {
                salt1,
                salt2,
                g,
                p,
            } => SrpInput { salt1, salt2, g, p },
            PasswordKdfAlgo::PasswordKdfAlgoUnknown => {
                return Err(Error::Unexpected(
                    "unsupported password KDF algorithm".to_owned(),
                ));
            }
        };
        let scalar = rtd_crypto::random_bytes(256);
        let answer = srp_hash(password, &srp_b, &scalar, &input)?;
        let query = Method::AuthCheckPassword(Box::new(AuthCheckPassword {
            password: Box::new(InputCheckPasswordSRP::InputCheckPasswordSRP {
                srp_id,
                a: answer.a,
                m1: answer.m1,
            }),
        }));
        match self.invoke(query)? {
            Object::AuthAuthorization(authorization) => Ok(*authorization),
            other => Err(unexpected("auth.checkPassword", &other)),
        }
    }

    /// Calls [`Client::check_password`] with the password from `provider`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unexpected`] when `provider` has no password, and the
    /// same errors as [`Client::check_password`].
    pub fn check_password_with(
        &mut self,
        provider: &dyn PasswordProvider,
    ) -> Result<AuthAuthorization> {
        let Some(password) = provider.password() else {
            return Err(Error::Unexpected(
                "two-factor password is required but none was provided".to_owned(),
            ));
        };
        self.check_password(&password)
    }

    /// Exports a QR login token (`auth.exportLoginToken`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with a token state other than a fresh token.
    pub fn export_login_token(
        &mut self,
        api_id: i32,
        api_hash: &str,
        except_ids: Vec<i64>,
    ) -> Result<LoginToken> {
        let query = Method::AuthExportLoginToken(Box::new(AuthExportLoginToken {
            api_id,
            api_hash: api_hash.to_owned(),
            except_ids,
        }));
        match self.invoke(query)? {
            Object::AuthLoginToken(token) => match *token {
                AuthLoginToken::LoginToken { expires, token } => {
                    Ok(LoginToken::new(expires, token))
                }
                other => Err(unexpected_token("auth.exportLoginToken", &other)),
            },
            other => Err(unexpected("auth.exportLoginToken", &other)),
        }
    }

    /// Imports an accepted QR login token (`auth.importLoginToken`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the token is not yet a successful authorization.
    pub fn import_login_token(&mut self, token: Vec<u8>) -> Result<AuthAuthorization> {
        let query = Method::AuthImportLoginToken(Box::new(AuthImportLoginToken { token }));
        match self.invoke(query)? {
            Object::AuthLoginToken(token) => match *token {
                AuthLoginToken::LoginTokenSuccess { authorization } => Ok(*authorization),
                other => Err(unexpected_token("auth.importLoginToken", &other)),
            },
            other => Err(unexpected("auth.importLoginToken", &other)),
        }
    }

    /// Accepts a scanned QR login token (`auth.acceptLoginToken`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server does not answer `Bool`.
    pub fn accept_login_token(&mut self, token: Vec<u8>) -> Result<()> {
        let query = Method::AuthAcceptLoginToken(Box::new(AuthAcceptLoginToken { token }));
        match self.invoke(query)? {
            Object::Bool(_) => Ok(()),
            other => Err(unexpected("auth.acceptLoginToken", &other)),
        }
    }

    /// Requests a password recovery code (`auth.requestPasswordRecovery`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with a different constructor.
    pub fn request_password_recovery(&mut self) -> Result<AuthPasswordRecovery> {
        let query = Method::AuthRequestPasswordRecovery(Box::new(AuthRequestPasswordRecovery));
        match self.invoke(query)? {
            Object::AuthPasswordRecovery(recovery) => Ok(*recovery),
            other => Err(unexpected("auth.requestPasswordRecovery", &other)),
        }
    }

    /// Checks a recovery code (`auth.checkRecoveryPassword`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server does not answer `Bool`.
    pub fn check_recovery_password(&mut self, code: &str) -> Result<()> {
        let query = Method::AuthCheckRecoveryPassword(Box::new(AuthCheckRecoveryPassword {
            code: code.to_owned(),
        }));
        match self.invoke(query)? {
            Object::Bool(_) => Ok(()),
            other => Err(unexpected("auth.checkRecoveryPassword", &other)),
        }
    }

    /// Recovers the password with a code (`auth.recoverPassword`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with a different constructor.
    pub fn recover_password(&mut self, code: &str) -> Result<AuthAuthorization> {
        let query = Method::AuthRecoverPassword(Box::new(AuthRecoverPassword {
            flags: 0,
            code: code.to_owned(),
            new_settings: None,
        }));
        match self.invoke(query)? {
            Object::AuthAuthorization(authorization) => Ok(*authorization),
            other => Err(unexpected("auth.recoverPassword", &other)),
        }
    }

    /// Authorizes a bot with a token (`auth.importBotAuthorization`).
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Unexpected`]
    /// when the server answers with a different constructor.
    pub fn bot_sign_in(
        &mut self,
        token: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<AuthAuthorization> {
        let query = Method::AuthImportBotAuthorization(Box::new(AuthImportBotAuthorization {
            flags: 0,
            api_id,
            api_hash: api_hash.to_owned(),
            bot_auth_token: token.to_owned(),
        }));
        match self.invoke(query)? {
            Object::AuthAuthorization(authorization) => Ok(*authorization),
            other => Err(unexpected("auth.importBotAuthorization", &other)),
        }
    }

    fn wrap_layer(&self, query: Method) -> Vec<u8> {
        let mut writer = Writer::new();
        Method::InvokeWithLayer(Box::new(InvokeWithLayer {
            layer: self.layer,
            query: Box::new(query),
        }))
        .encode(&mut writer);
        writer.into_vec()
    }

    /// Reads and processes one inbound frame without a pending call.
    ///
    /// Returns any update the frame carried, so a caller can drain unsolicited
    /// updates between calls by looping until it returns `Ok(None)` (or would
    /// block). Results for no pending request are discarded.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto or decode error.
    pub fn poll(&mut self) -> Result<Option<Updates>> {
        let frame = self.transport.recv_frame()?;
        let decrypted = self.session.decode(&frame)?;
        self.flush_acks()?;
        let id = Reader::new(&decrypted.body)
            .peek_id()
            .map_err(|error| Error::Decode(error.to_string()))?;
        let is_result = matches!(
            id,
            RPC_RESULT_TYPE_ID
                | MESSAGE_CONTAINER_TYPE_ID
                | GZIP_TYPE_ID
                | BAD_SERVER_SALT_ID
                | BAD_MSG_NOTIFICATION_ID
        );
        if !is_result && let Some(updates) = Self::decode_update(&decrypted.body) {
            return Ok(Some(updates));
        }
        let _ = self.handle_body(0, &decrypted.body)?;
        Ok(None)
    }

    /// Requests `get_future_salts` and stores the returned salts.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto or decode error, or [`Error::NoResult`] when
    /// the reply does not arrive within the bounded read window.
    pub fn request_future_salts(&mut self, num: i32) -> Result<()> {
        let mut writer = Writer::new();
        ServiceMethod::Get_future_salts(Box::new(Get_future_salts { num })).encode(&mut writer);
        let body = writer.into_vec();
        let message = self.session.allocate_content();
        self.send(message, &body)?;

        for _ in 0..MAX_ROUNDS {
            let frame = self.transport.recv_frame()?;
            let decrypted = self.session.decode(&frame)?;
            self.flush_acks()?;
            if Reader::new(&decrypted.body).peek_id().ok() == Some(FUTURE_SALTS_ID) {
                let mut decoder =
                    ServiceDecoder::new(&decrypted.body, ServiceDecodeLimits::default());
                if let Ok(ServiceObject::FutureSalts(salts)) = ServiceObject::decode(&mut decoder) {
                    let FutureSalts::Future_salts { salts, .. } = *salts;
                    for salt in salts {
                        let FutureSalt::Future_salt {
                            valid_since,
                            valid_until,
                            salt,
                        } = salt;
                        self.session.insert_future_salt(
                            i64::from(valid_since),
                            i64::from(valid_until),
                            salt,
                        );
                    }
                    return Ok(());
                }
                continue;
            }
            let _ = self.handle_body(0, &decrypted.body)?;
        }
        Err(Error::NoResult { message_id: 0 })
    }

    /// Advances the active salt to the newest one valid at `now` (Unix seconds).
    ///
    /// Returns whether the active salt changed.
    pub fn advance_salts(&mut self, now: i64) -> bool {
        self.session.advance_salts(now)
    }

    /// Sends a service `ping` and returns once the matching `pong` arrives.
    ///
    /// This is the primitive a keepalive loop drives; it does not schedule
    /// timers itself.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto or decode error, or [`Error::NoResult`] when
    /// no `pong` arrives within the bounded read window.
    pub fn ping(&mut self, ping_id: i64) -> Result<()> {
        let mut writer = Writer::new();
        ServiceMethod::Ping(Box::new(Ping { ping_id })).encode(&mut writer);
        let body = writer.into_vec();
        let message = self.session.allocate_service();
        self.send(message, &body)?;

        for _ in 0..MAX_ROUNDS {
            let frame = self.transport.recv_frame()?;
            let decrypted = self.session.decode(&frame)?;
            self.flush_acks()?;
            if Reader::new(&decrypted.body).peek_id().ok() == Some(Pong::TYPE_ID) {
                let mut decoder =
                    ServiceDecoder::new(&decrypted.body, ServiceDecodeLimits::default());
                if let Ok(ServiceObject::Pong(pong)) = ServiceObject::decode(&mut decoder)
                    && pong.ping_id == ping_id
                {
                    return Ok(());
                }
                continue;
            }
            // Process unrelated frames (results, containers, updates) as usual.
            let _ = self.handle_body(0, &decrypted.body)?;
        }
        Err(Error::NoResult {
            message_id: ping_id,
        })
    }

    fn send(&mut self, message: MessageSeq, body: &[u8]) -> Result<()> {
        let frame = self.session.encode(message, body, &mut self.random);
        self.transport.send_frame(&frame)
    }

    fn call(&mut self, body: Vec<u8>) -> Result<Object> {
        let mut last = 0;
        for _ in 0..MAX_ATTEMPTS {
            let message = self.session.allocate_content();
            last = message.msg_id;
            self.send(message, &body)?;
            if let Some(object) = self.await_result(message.msg_id)? {
                return Ok(object);
            }
        }
        Err(Error::NoResult { message_id: last })
    }

    fn await_result(&mut self, request_id: i64) -> Result<Option<Object>> {
        for _ in 0..MAX_ROUNDS {
            let frame = self.transport.recv_frame()?;
            let decrypted = self.session.decode(&frame)?;
            self.flush_acks()?;
            match self.handle_body(request_id, &decrypted.body)? {
                Reply::Result(object) => return Ok(Some(object)),
                Reply::BadSalt => return Ok(None),
                Reply::Ignored => continue,
            }
        }
        Err(Error::NoResult {
            message_id: request_id,
        })
    }

    fn flush_acks(&mut self) -> Result<()> {
        let ids = self.session.take_acks();
        if ids.is_empty() {
            return Ok(());
        }
        let mut writer = Writer::new();
        MsgsAck::Msgs_ack { msg_ids: ids }.encode(&mut writer);
        let body = writer.into_vec();
        let message = self.session.allocate_service();
        self.send(message, &body)
    }

    fn handle_body(&mut self, request_id: i64, body: &[u8]) -> Result<Reply> {
        let id = Reader::new(body)
            .peek_id()
            .map_err(|error| Error::Decode(error.to_string()))?;
        match id {
            RPC_RESULT_TYPE_ID => {
                let result =
                    RpcResult::decode(body).map_err(|error| Error::Decode(error.to_string()))?;
                if result.request_message_id != request_id {
                    return Ok(Reply::Ignored);
                }
                let payload = self.ungzip(&result.result)?;
                self.decode_result(&payload).map(Reply::Result)
            }
            MESSAGE_CONTAINER_TYPE_ID => {
                let container = MessageContainer::from_bytes(body)
                    .map_err(|error| Error::Decode(error.to_string()))?;
                for message in container.messages {
                    if let Reply::Result(object) = self.handle_body(request_id, &message.body)? {
                        return Ok(Reply::Result(object));
                    }
                }
                Ok(Reply::Ignored)
            }
            GZIP_TYPE_ID => {
                let inner = self.ungzip(body)?;
                self.handle_body(request_id, &inner)
            }
            BAD_SERVER_SALT_ID => {
                let mut decoder = ServiceDecoder::new(body, ServiceDecodeLimits::default());
                if let Ok(BadMsgNotification::Bad_server_salt {
                    new_server_salt, ..
                }) = BadMsgNotification::decode(&mut decoder)
                {
                    self.session.set_salt(new_server_salt);
                }
                Ok(Reply::BadSalt)
            }
            BAD_MSG_NOTIFICATION_ID => Ok(Reply::Ignored),
            _ => {
                self.dispatch_update(body);
                Ok(Reply::Ignored)
            }
        }
    }

    /// Offers a non-result message to the update handler; returns whether it was
    /// recognised as an update.
    fn dispatch_update(&mut self, body: &[u8]) -> bool {
        if self.handler.is_none() {
            return false;
        }
        let Some(updates) = Self::decode_update(body) else {
            return false;
        };
        if let Some(handler) = self.handler.as_mut() {
            handler(updates);
        }
        true
    }

    /// Decodes a body as updates, wrapping a bare update in `UpdateShort`.
    fn decode_update(body: &[u8]) -> Option<Updates> {
        let mut decoder = Decoder::new(body, DecodeLimits::default());
        if let Ok(updates) = Updates::decode(&mut decoder) {
            return Some(updates);
        }
        let mut decoder = Decoder::new(body, DecodeLimits::default());
        match Update::decode(&mut decoder) {
            Ok(update) => Some(Updates::UpdateShort {
                update: Box::new(update),
                date: 0,
            }),
            Err(_) => None,
        }
    }

    fn ungzip(&self, payload: &[u8]) -> Result<Vec<u8>> {
        if Reader::new(payload).peek_id().ok() == Some(GZIP_TYPE_ID) {
            let gzip = Gzip::decode(payload).map_err(|error| Error::Decode(error.to_string()))?;
            Ok(gzip.data)
        } else {
            Ok(payload.to_vec())
        }
    }

    fn decode_result(&self, payload: &[u8]) -> Result<Object> {
        let mut decoder = Decoder::new(payload, DecodeLimits::default());
        if let Ok(object) = Object::decode(&mut decoder) {
            return Ok(object);
        }
        // Otherwise it is an MTProto `rpc_error`.
        let mut service = ServiceDecoder::new(payload, ServiceDecodeLimits::default());
        match ServiceObject::decode(&mut service) {
            Ok(ServiceObject::RpcError(error)) => match *error {
                RpcError::Rpc_error {
                    error_code,
                    error_message,
                } => Err(Error::Rpc(rtd_error::Error::new(error_code, error_message))),
            },
            _ => Err(Error::Decode("unrecognised RPC result".to_owned())),
        }
    }
}

/// An error for a login-token call that answered with an unexpected token state.
fn unexpected_token(what: &str, token: &AuthLoginToken) -> Error {
    Error::Unexpected(format!(
        "{what} returned unexpected login-token state {:#010x}",
        token.type_id()
    ))
}

/// An error for a method that answered with a constructor the wrapper expects
/// differently.
fn unexpected(what: &str, object: &Object) -> Error {
    Error::Unexpected(format!(
        "{what} returned unexpected constructor {:#010x}",
        object.type_id()
    ))
}

/// Adapts a [`Transport`] to the synchronous `rtd_exchange::Stream` the
/// authorization-key exchange expects.
struct ExchangeAdapter<'a, T: Transport>(&'a mut T);

impl<T: Transport> ExchangeStream for ExchangeAdapter<'_, T> {
    fn send(&mut self, frame: &[u8]) -> rtd_exchange::Result<()> {
        self.0
            .send_frame(frame)
            .map_err(|error| rtd_exchange::Error::Stream(error.to_string()))
    }

    fn recv(&mut self) -> rtd_exchange::Result<Vec<u8>> {
        self.0
            .recv_frame()
            .map_err(|error| rtd_exchange::Error::Stream(error.to_string()))
    }
}
