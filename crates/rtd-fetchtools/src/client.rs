//! Official-Telegram client bootstrap and a datacenter-aware wrapper.
//!
//! `rtd-client` is a library: it exposes a synchronous [`Client`] over a
//! transport, but leaves the production tuning (public keys, resolver, device
//! parameters, session persistence) to the caller. This module provides exactly
//! that for the fetchers, plus a thin [`Tg`] wrapper that follows
//! `*_MIGRATE_*` errors, so a document stored on a file DC downloads from there.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rtd_client::dcs::{Resolver as _, StaticResolver, migrate_target};
use rtd_client::storage::{FileStorage, MemoryStorage, Storage};
use rtd_client::{Client, Downloader, Framed, InitParams, ReadWrite, TcpTransport, connect_tcp};
use rtd_crypto::parse_rsa_public_keys;
use rtd_crypto::rsa::RsaPublicKey;
use rtd_exchange::ClientConfig;
use rtd_proto::CodecKind;
use rtd_tg::{
    AuthAuthorization, AuthSentCode, CodeSettings, InputFileLocation, Method, Object, User,
};
use rtd_util::{Clock, SystemClock};

use crate::Result;
use crate::creds;
use crate::proxy::Proxy;

/// The default authorization datacenter.
pub const DEFAULT_DC: i32 = 2;

/// The bundled official Telegram production RSA public keys.
///
/// These are the current keys Telegram publishes for the authorization-key
/// exchange (the two-key set used by TDesktop and `td`). `rtd`'s exchange picks
/// whichever matches the fingerprint the server returns.
pub const PROD_PUBLIC_KEYS: &str = "\
-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAyMEdY1aR+sCR3ZSJrtztKTKqigvO/vBfqACJLZtS7QMgCGXJ6XIR
yy7mx66W0/sOFa7/1mAZtEoIokDP3ShoqF4fVNb6XeqgQfaUHd8wJpDWHcR2OFwv
plUUI1PLTktZ9uW2WE23b+ixNwJjJGwBDJPQEQFBE+vfmH0JP503wr5INS1poWg/
j25sIWeYPHYeOrFp/eXaqhISP6G+q2IeTaWTXpwZj4LzXq5YOpk4bYEQ6mvRq7D1
aHWfYmlEGepfaYR8Q0YqvvhYtMte3ITnuSJs171+GDqpdKcSwHnd6FudwGO4pcCO
j4WcDuXc2CTHgH8gFTNhp/Y8/SpDOhvn9QIDAQAB
-----END RSA PUBLIC KEY-----
-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEA6LszBcC1LGzyr992NzE0ieY+BSaOW622Aa9Bd4ZHLl+TuFQ4lo4g
5nKaMBwK/BIb9xUfg0Q29/2mgIR6Zr9krM7HjuIcCzFvDtr+L0GQjae9H0pRB2OO
62cECs5HKhT5DZ98K33vmWiLowc621dQuwKWSQKjWf50XYFw42h21P2KXUGyp2y/
+aEyZ+uVgLLQbRA1dEjSDZ2iGRy12Mk5gpYc397aYp438fsJoHIgJ2lgMv5h7WY9
t6N/byY9Nw9p21Og3AoXSL2q/2IJ1WRUhebgAdGVMlV1fkuOQoEzR7EdpqtQD9Cs
5+bfo3Nhmcyvk5ftB0WkJ9z6bNZ7yxrP8wIDAQAB
-----END RSA PUBLIC KEY-----
";

/// A client bound to official Telegram, with session persistence and DC routing.
pub struct Tg {
    api_id: i32,
    api_hash: String,
    storage: Box<dyn Storage>,
    session_path: Option<PathBuf>,
    keys: Vec<RsaPublicKey>,
    clock: Arc<dyn Clock + Send + Sync>,
    home_dc: i32,
    home: Client<TcpTransport>,
    others: BTreeMap<i32, Client<TcpTransport>>,
    proxy: Option<Proxy>,
}

impl Tg {
    /// Connects to official Telegram using `session_path` for session storage.
    ///
    /// A stored session is resumed without a fresh key exchange; otherwise a new
    /// authorization key is negotiated and persisted.
    ///
    /// # Errors
    ///
    /// Returns an error when the keys cannot be parsed, the transport fails, the
    /// key exchange fails, or the session file is unreadable.
    pub fn connect(api_id: i32, api_hash: &str, session_path: impl Into<PathBuf>) -> Result<Self> {
        Self::connect_to(api_id, api_hash, session_path, DEFAULT_DC)
    }

    /// Connects to a specific home datacenter.
    ///
    /// # Errors
    ///
    /// As [`Tg::connect`].
    pub fn connect_to(
        api_id: i32,
        api_hash: &str,
        session_path: impl Into<PathBuf>,
        home_dc: i32,
    ) -> Result<Self> {
        let path = session_path.into();
        let storage: Box<dyn Storage> = Box::new(FileStorage::new(&path));
        Self::with_storage(api_id, api_hash, storage, Some(path), home_dc, None)
    }

    /// Connects to a specific home datacenter through an optional SOCKS5 proxy.
    ///
    /// # Errors
    ///
    /// As [`Tg::connect`].
    pub fn connect_proxied(
        api_id: i32,
        api_hash: &str,
        session_path: impl Into<PathBuf>,
        home_dc: i32,
        proxy: Option<Proxy>,
    ) -> Result<Self> {
        let path = session_path.into();
        let storage: Box<dyn Storage> = Box::new(FileStorage::new(&path));
        Self::with_storage(api_id, api_hash, storage, Some(path), home_dc, proxy)
    }

    /// Connects with an in-memory session, mirroring Go's session-less client.
    ///
    /// No session is persisted; a fresh anonymous authorization is negotiated on
    /// every run.
    ///
    /// # Errors
    ///
    /// As [`Tg::connect_to`].
    pub fn ephemeral(api_id: i32, api_hash: &str, home_dc: i32) -> Result<Self> {
        let storage: Box<dyn Storage> = Box::new(MemoryStorage::new());
        Self::with_storage(api_id, api_hash, storage, None, home_dc, None)
    }

    fn with_storage(
        api_id: i32,
        api_hash: &str,
        storage: Box<dyn Storage>,
        session_path: Option<PathBuf>,
        home_dc: i32,
        proxy: Option<Proxy>,
    ) -> Result<Self> {
        let keys = parse_rsa_public_keys(PROD_PUBLIC_KEYS)
            .map_err(|error| format!("parse production public keys: {error}"))?;
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock);
        let existing = storage.load()?;
        let mut home = open_client(
            home_dc,
            &keys,
            existing.as_ref(),
            clock.clone(),
            proxy.as_ref(),
        )?;
        home.initialize(init_params(api_id))?;
        storage.store(&home.session_data())?;
        Ok(Self {
            api_id,
            api_hash: api_hash.to_owned(),
            storage,
            session_path,
            keys,
            clock,
            home_dc,
            home,
            others: BTreeMap::new(),
            proxy,
        })
    }

    /// Returns the session file path, if the client persists one.
    #[must_use]
    pub fn session_path(&self) -> Option<&Path> {
        self.session_path.as_deref()
    }

    /// Returns the application id.
    #[must_use]
    pub fn api_id(&self) -> i32 {
        self.api_id
    }

    /// Returns the application hash.
    #[must_use]
    pub fn api_hash(&self) -> &str {
        &self.api_hash
    }

    /// Persists the home session to disk.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be written.
    pub fn save(&self) -> Result<()> {
        self.storage.store(&self.home.session_data())?;
        Ok(())
    }

    /// Reports whether the session is authorized (`users.getUsers` self).
    ///
    /// # Errors
    ///
    /// Returns an error for non-`401` RPC failures and transport errors.
    pub fn is_authorized(&mut self) -> Result<bool> {
        match self.raw_call(|client| client.self_user()) {
            Ok(_) => Ok(true),
            Err(rtd_client::Error::Rpc(error)) if error.code() == 401 => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Returns the authorized account's user.
    ///
    /// # Errors
    ///
    /// Returns a transport or RPC error.
    pub fn self_user(&mut self) -> Result<User> {
        self.raw_call(|client| client.self_user())
            .map_err(Into::into)
    }

    /// Sends a login code (`auth.sendCode`), following a phone migration.
    ///
    /// # Errors
    ///
    /// Returns a transport or RPC error.
    pub fn send_code(&mut self, phone: &str) -> Result<AuthSentCode> {
        let settings = default_settings();
        let hash = self.api_hash.clone();
        let id = self.api_id;
        self.raw_call(|client| client.send_code(phone, id, &hash, settings.clone()))
            .map_err(Into::into)
    }

    /// Signs in with a code (`auth.signIn`), following a phone migration.
    ///
    /// # Errors
    ///
    /// Returns a transport or RPC error.
    pub fn sign_in(
        &mut self,
        phone: &str,
        phone_code_hash: &str,
        code: &str,
    ) -> Result<AuthAuthorization> {
        self.raw_call(|client| client.sign_in(phone, phone_code_hash, Some(code)))
            .map_err(Into::into)
    }

    /// Completes two-factor sign-in (`account.getPassword` + `auth.checkPassword`).
    ///
    /// # Errors
    ///
    /// Returns a transport or RPC error.
    pub fn check_password(&mut self, password: &[u8]) -> Result<AuthAuthorization> {
        self.raw_call(|client| client.check_password(password))
            .map_err(Into::into)
    }

    /// Invokes one canonical-layer method, following a datacenter migration.
    ///
    /// # Errors
    ///
    /// Returns a transport or RPC error.
    pub fn invoke(&mut self, method: Method) -> Result<Object> {
        self.raw_call(|client| client.invoke(method.clone()))
            .map_err(Into::into)
    }

    /// Invokes one canonical-layer method and returns a bare-vector result.
    ///
    /// # Errors
    ///
    /// Returns a transport or RPC error, or an error when the result is not a
    /// vector.
    pub fn invoke_vector(&mut self, method: Method) -> Result<Vec<Object>> {
        match self.raw_call(|client| client.invoke_raw(method.clone()))? {
            rtd_client::InvokeResult::Vector(items) => Ok(items),
            rtd_client::InvokeResult::Object(_) => Err("method did not return a vector".into()),
        }
    }

    /// Downloads `size` bytes of `location` from the datacenter `dc`.
    ///
    /// # Errors
    ///
    /// Returns a transport, RPC or decode error.
    pub fn download(
        &mut self,
        location: InputFileLocation,
        size: usize,
        dc: i32,
    ) -> Result<Vec<u8>> {
        let downloader = Downloader::new();
        let mut dc = dc;
        loop {
            let client = self.client_for(dc)?;
            match downloader.download(client, location.clone(), size) {
                Ok(bytes) => return Ok(bytes),
                Err(rtd_client::Error::Rpc(error)) => {
                    if let Some(target) = migrate_target(&error) {
                        dc = target;
                        continue;
                    }
                    return Err(rtd_client::Error::Rpc(error).into());
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Runs `op` against the home client, following `*_MIGRATE_*` errors.
    fn raw_call<T>(
        &mut self,
        mut op: impl FnMut(&mut Client<TcpTransport>) -> rtd_client::Result<T>,
    ) -> rtd_client::Result<T> {
        let mut dc = self.home_dc;
        loop {
            let client = self.client_for(dc)?;
            match op(client) {
                Ok(value) => return Ok(value),
                Err(rtd_client::Error::Rpc(error)) => {
                    if let Some(target) = migrate_target(&error) {
                        dc = target;
                        continue;
                    }
                    return Err(rtd_client::Error::Rpc(error));
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Returns the client for `dc`, exporting/importing authorization on demand.
    fn client_for(&mut self, dc: i32) -> rtd_client::Result<&mut Client<TcpTransport>> {
        if dc == self.home_dc {
            return Ok(&mut self.home);
        }
        if !self.others.contains_key(&dc) {
            let exported = self.home.export_authorization(dc)?;
            let mut fresh = open_client(
                dc,
                &self.keys,
                None,
                self.clock.clone(),
                self.proxy.as_ref(),
            )?;
            fresh.initialize(init_params(self.api_id))?;
            fresh.import_authorization(exported.id, exported.bytes)?;
            self.others.insert(dc, fresh);
        }
        self.others.get_mut(&dc).ok_or(rtd_client::Error::Closed)
    }
}

/// Builds a client for `dc`, resuming `existing` when present.
fn open_client(
    dc: i32,
    keys: &[RsaPublicKey],
    existing: Option<&rtd_client::SessionData>,
    clock: Arc<dyn Clock + Send + Sync>,
    proxy: Option<&Proxy>,
) -> rtd_client::Result<Client<TcpTransport>> {
    let resolver = StaticResolver::production();
    let endpoint = resolver
        .resolve(dc)
        .and_then(|datacenter| datacenter.endpoints.first())
        .cloned()
        .ok_or_else(|| {
            rtd_client::Error::Unexpected(format!("no known endpoint for datacenter {dc}"))
        })?;
    let transport = build_transport(&endpoint.address, dc, proxy)?;
    if let Some(data) = existing {
        return Ok(Client::from_session(transport, data, clock));
    }
    let config = ClientConfig {
        dc,
        temporary: false,
        expires_in: 0,
        public_keys: keys.to_vec(),
    };
    Client::connect(transport, &config, clock)
}

/// Builds an obfuscated2 transport, optionally through a SOCKS5 proxy.
fn build_transport(
    address: &str,
    dc: i32,
    proxy: Option<&Proxy>,
) -> rtd_client::Result<TcpTransport> {
    match proxy {
        None => connect_tcp(address, dc as u16, CodecKind::Intermediate, Some(&[])),
        Some(proxy) => {
            let stream = crate::proxy::connect(address, proxy)?;
            let tag = CodecKind::Intermediate
                .obfuscated_tag()
                .ok_or_else(|| rtd_client::Error::Unexpected("no obfuscation tag".to_owned()))?;
            let obfuscated = rtd_transport::Obfuscated2::handshake(stream, tag, dc as u16, &[])
                .map_err(|error| rtd_client::Error::Unexpected(error.to_string()))?;
            Framed::new(
                Box::new(obfuscated) as Box<dyn ReadWrite>,
                CodecKind::Intermediate,
                false,
            )
        }
    }
}

/// Returns `initConnection` device parameters for the fetchers.
fn init_params(api_id: i32) -> InitParams {
    InitParams {
        api_id,
        device_model: "rtd-fetchtools".to_owned(),
        system_version: std::env::consts::OS.to_owned(),
        app_version: "0.1.1".to_owned(),
        system_lang_code: "en".to_owned(),
        lang_pack: String::new(),
        lang_code: "en".to_owned(),
    }
}

/// Returns login code settings with no special options.
fn default_settings() -> CodeSettings {
    CodeSettings {
        flags: 0,
        allow_flashcall: false,
        current_number: false,
        allow_app_hash: false,
        allow_missed_call: false,
        allow_firebase: false,
        unknown_number: false,
        logout_tokens: None,
        token: None,
        app_sandbox: None,
    }
}

/// Resolves the session path the same way the Go tools do.
#[must_use]
pub fn default_session_path() -> String {
    creds::session_path("/tmp/appearance.session")
}

/// Reports whether a crate error is the RPC `SESSION_PASSWORD_NEEDED`.
#[must_use]
pub fn is_password_needed(error: &crate::Error) -> bool {
    if let Some(rtd_client::Error::Rpc(rpc)) = error.downcast_ref::<rtd_client::Error>() {
        return rpc.is_type("SESSION_PASSWORD_NEEDED");
    }
    false
}
