//! The per-connection actor, service messages and admission budgets.
//!
//! Owned by task group 8. Implemented in this file.
//!
//! Every accepted connection becomes one actor task. The socket is split with
//! [`tokio::io::split`]: a reader task frames inbound messages and a single
//! *writer task* owns the write half, assigns message identifiers and sequence
//! numbers through [`rtd_proto::session::Session`], encrypts and serializes every
//! outbound frame, so there are no locks on the hot path (design D6/D20).
//!
//! The server side of the authorization-key exchange is driven over the same
//! connection through a synchronous [`rtd_exchange::Stream`] bridged to the
//! async socket with channels and a blocking task; the resulting key is saved to
//! the core *before* the final `dh_gen_ok` is written. Encrypted frames are then
//! accepted by authorization-key id, decrypted with
//! [`rtd_crypto::encrypted_message`], resolved from the core when the edge does
//! not hold the key, and answered with an authorization-key-not-found protocol
//! error when even the core does not.
//!
//! # Supplying the server key
//!
//! The exchange implementation is supplied through the [`Exchange`] trait. That
//! keeps the actor free of an `rsa` dependency (the key type lives in
//! `rtd-exchange`, not here): the `rtd-edge` binary implements [`Exchange`] with
//! its configured [`rtd_exchange::ServerConfig`] key, and tests implement it with
//! a fake. A blanket implementation lets a `Fn` closure serve as an [`Exchange`].

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use rtd_bin::Writer;
use rtd_crypto::encrypted_message::{EncryptedMessage, MessageHeader, auth_key_id};
use rtd_crypto::{AuthKey, Error as CryptoError, Side, SystemRandom};
use rtd_error::Error as RtdError;
use rtd_exchange::Stream as ExchangeStream;
use rtd_mt::{
    BadMsgNotification, DecodeLimits as MtDecodeLimits, Decoder as MtDecoder, Destroy_session,
    DestroySessionRes, FutureSalt, FutureSalts, Get_future_salts, MsgResendReq, MsgsAck,
    MsgsStateInfo, MsgsStateReq, NewSession, Ping, Ping_delay_disconnect, Pong, RpcError,
};
use rtd_proto::codec::{
    Abridged, CODE_AUTH_KEY_NOT_FOUND, Codec, CodecKind, Full, Intermediate, MAX_MESSAGE_SIZE,
    PaddedIntermediate,
};
use rtd_proto::message_id::MessageType;
use rtd_proto::{
    DisconnectReason, Inbound, MessageClass, OutboundMessage, Session, SessionError,
    UnencryptedMessage,
};
use rtd_tlprofile::{Limits, Profile};
use rtd_util::Clock;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::core::{AdmittedCall, AuthKeyId, Core, SessionId};
use crate::ingress::{ConnectionMeta, Ingress, IngressConfig, IngressError, TransportMode};
use crate::session::{self, AuthKeyCache, SessionKey, SessionRegistry};

/// Default bound on the actor's inbound frame queue.
const INBOUND_QUEUE: usize = 64;
/// Default bound on the writer's command queue.
const WRITER_QUEUE: usize = 128;

/// `msgs_ack` constructor identifier.
const ID_MSGS_ACK: u32 = 0x62d6_b459;
/// `msgs_state_req` constructor identifier.
const ID_MSGS_STATE_REQ: u32 = 0xda69_fb52;
/// `get_future_salts` constructor identifier.
const ID_GET_FUTURE_SALTS: u32 = Get_future_salts::TYPE_ID;
/// `msg_resend_req` constructor identifier.
const ID_MSG_RESEND_REQ: u32 = 0x7d86_1a08;
/// `destroy_session` constructor identifier.
const ID_DESTROY_SESSION: u32 = Destroy_session::TYPE_ID;
/// `bad_msg_notification` constructor identifier.
const ID_BAD_MSG_NOTIFICATION: u32 = 0xa7ef_f811;
/// `bad_server_salt` constructor identifier.
const ID_BAD_SERVER_SALT: u32 = 0xedab_447b;
/// `set_client_DH_params_answer`/`dh_gen_ok` constructor identifier.
const ID_DH_GEN_OK: u32 = 0x3bcb_f734;
/// `set_client_DH_params_answer`/`dh_gen_retry` constructor identifier.
const ID_DH_GEN_RETRY: u32 = 0x46dc_1fb9;
/// `set_client_DH_params_answer`/`dh_gen_fail` constructor identifier.
const ID_DH_GEN_FAIL: u32 = 0xa69d_ae02;

/// A caller-supplied source identity for the per-source connection budget.
///
/// The [`Ingress`] abstraction yields only [`ConnectionMeta`], not the peer
/// address, so the per-source budget is keyed by the transport identity present
/// in the metadata (the MTProxy user when there is one, otherwise the DC label).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceKey(pub u64);

impl SourceKey {
    /// Derives a source key from an accepted connection's metadata.
    #[must_use]
    pub fn from_meta(meta: &ConnectionMeta) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut mix = |value: u64| {
            hash = (hash ^ value).wrapping_mul(0x0000_0100_0000_01b3);
        };
        if let Some(dc) = meta.dc_id {
            mix(u64::from(dc));
        }
        if let Some(user) = &meta.proxy_user {
            for byte in user.as_bytes() {
                mix(u64::from(*byte));
            }
        }
        mix(match meta.mode {
            TransportMode::Plain => 0,
            TransportMode::Obfuscated => 1,
            TransportMode::FakeTls => 2,
            TransportMode::WebSocket => 3,
        });
        Self(hash)
    }
}

/// The admission budgets enforced before and during a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetLimits {
    /// Maximum number of concurrently admitted connections.
    pub max_connections: usize,
    /// Maximum number of concurrently admitted connections per source.
    pub max_per_source: usize,
    /// Maximum number of concurrent key exchanges.
    pub max_handshakes: usize,
    /// Maximum aggregate bytes of frames being processed across all connections.
    pub max_frame_bytes: usize,
}

impl Default for BudgetLimits {
    fn default() -> Self {
        Self {
            max_connections: 4096,
            max_per_source: 64,
            max_handshakes: 256,
            max_frame_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Why an admission request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The total connection budget is exhausted.
    Connections,
    /// The per-source connection budget is exhausted.
    PerSource,
    /// The concurrent handshake budget is exhausted.
    Handshakes,
    /// The aggregate frame-byte budget is exhausted.
    FrameBytes,
}

#[derive(Debug)]
struct BudgetState {
    connections: usize,
    handshakes: usize,
    frames: usize,
    per_source: HashMap<u64, usize>,
}

/// The shared admission budgets.
///
/// Admission is a short, synchronous check; refusing a connection never touches
/// an admitted one, so load-shedding leaves admitted sessions running.
#[derive(Debug)]
pub struct Budgets {
    limits: BudgetLimits,
    state: Mutex<BudgetState>,
}

impl Budgets {
    /// Creates budgets with `limits`.
    #[must_use]
    pub fn new(limits: BudgetLimits) -> Self {
        Self {
            limits,
            state: Mutex::new(BudgetState {
                connections: 0,
                handshakes: 0,
                frames: 0,
                per_source: HashMap::new(),
            }),
        }
    }

    /// Returns the configured limits.
    #[must_use]
    pub const fn limits(&self) -> BudgetLimits {
        self.limits
    }

    /// Admits one connection from `source`.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Connections`] or [`Refusal::PerSource`] when the
    /// matching budget is exhausted.
    pub fn admit(self: &Arc<Self>, source: SourceKey) -> Result<Admission, Refusal> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.connections >= self.limits.max_connections {
            return Err(Refusal::Connections);
        }
        let per_source = state.per_source.entry(source.0).or_insert(0);
        if *per_source >= self.limits.max_per_source {
            return Err(Refusal::PerSource);
        }
        *per_source += 1;
        state.connections += 1;
        drop(state);
        Ok(Admission {
            budgets: self.clone(),
            source,
        })
    }

    /// Begins one key exchange.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Handshakes`] when the concurrent handshake budget is
    /// exhausted.
    pub fn begin_handshake(self: &Arc<Self>) -> Result<HandshakeGuard, Refusal> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.handshakes >= self.limits.max_handshakes {
            return Err(Refusal::Handshakes);
        }
        state.handshakes += 1;
        drop(state);
        Ok(HandshakeGuard {
            budgets: self.clone(),
        })
    }

    /// Reserves `bytes` of the aggregate frame-byte budget.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::FrameBytes`] when the aggregate budget would be
    /// exceeded.
    pub fn reserve_frame(self: &Arc<Self>, bytes: usize) -> Result<FrameGuard, Refusal> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.frames.saturating_add(bytes) > self.limits.max_frame_bytes {
            return Err(Refusal::FrameBytes);
        }
        state.frames += bytes;
        drop(state);
        Ok(FrameGuard {
            budgets: self.clone(),
            bytes,
        })
    }

    /// Returns the number of admitted connections.
    #[must_use]
    pub fn connections(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .connections
    }

    /// Returns the number of concurrent handshakes.
    #[must_use]
    pub fn handshakes(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .handshakes
    }

    /// Returns the number of reserved frame bytes.
    #[must_use]
    pub fn frame_bytes(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .frames
    }

    fn release_connection(&self, source: SourceKey) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.connections = state.connections.saturating_sub(1);
        if let Some(count) = state.per_source.get_mut(&source.0) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                state.per_source.remove(&source.0);
            }
        }
    }

    fn release_handshake(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.handshakes = state.handshakes.saturating_sub(1);
    }

    fn release_frame(&self, bytes: usize) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.frames = state.frames.saturating_sub(bytes);
    }
}

/// A held connection admission; dropping it releases the budget.
#[derive(Debug)]
pub struct Admission {
    budgets: Arc<Budgets>,
    source: SourceKey,
}

impl Drop for Admission {
    fn drop(&mut self) {
        self.budgets.release_connection(self.source);
    }
}

/// A held handshake admission; dropping it releases the budget.
#[derive(Debug)]
pub struct HandshakeGuard {
    budgets: Arc<Budgets>,
}

impl Drop for HandshakeGuard {
    fn drop(&mut self) {
        self.budgets.release_handshake();
    }
}

/// A held frame-byte reservation; dropping it releases the budget.
#[derive(Debug)]
pub struct FrameGuard {
    budgets: Arc<Budgets>,
    bytes: usize,
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        self.budgets.release_frame(self.bytes);
    }
}

/// Runs the server side of the authorization-key exchange.
///
/// The actor invokes this on a blocking thread with a synchronous frame stream;
/// implementations own the RSA key and call
/// [`rtd_exchange::server_exchange`]. A blanket implementation is provided for
/// closures.
pub trait Exchange: Send + Sync {
    /// Runs the exchange to completion over `stream`.
    ///
    /// # Errors
    ///
    /// Returns the exchange's error; the actor fails the connection closed.
    fn exchange(
        &self,
        stream: &mut dyn ExchangeStream,
        clock: &dyn Clock,
        random: &mut dyn rtd_crypto::RandomSource,
    ) -> rtd_exchange::Result<rtd_exchange::ServerExchangeResult>;
}

impl<F> Exchange for F
where
    F: Fn(
            &mut dyn ExchangeStream,
            &dyn Clock,
            &mut dyn rtd_crypto::RandomSource,
        ) -> rtd_exchange::Result<rtd_exchange::ServerExchangeResult>
        + Send
        + Sync,
{
    fn exchange(
        &self,
        stream: &mut dyn ExchangeStream,
        clock: &dyn Clock,
        random: &mut dyn rtd_crypto::RandomSource,
    ) -> rtd_exchange::Result<rtd_exchange::ServerExchangeResult> {
        self(stream, clock, random)
    }
}

/// Configuration shared by every connection actor.
pub struct ServerOptions {
    /// The datacenter this server serves.
    pub dc: i32,
    /// The injected clock used for message identifiers and timeouts.
    pub clock: Arc<dyn Clock + Send + Sync>,
    /// The strategy driving the server side of the key exchange.
    pub exchange: Arc<dyn Exchange>,
    /// The default negotiated layer used for admission and result encoding.
    pub layer: Profile,
    /// Idle timeout after which a transient session record is evicted.
    pub idle_timeout: Duration,
    /// The admission budgets applied to accepted connections.
    pub budget_limits: BudgetLimits,
}

impl std::fmt::Debug for ServerOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerOptions")
            .field("dc", &self.dc)
            .field("layer", &self.layer)
            .field("idle_timeout", &self.idle_timeout)
            .field("budget_limits", &self.budget_limits)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct ConnContext {
    core: Arc<dyn Core>,
    options: Arc<ServerOptions>,
    budgets: Arc<Budgets>,
    registry: Arc<SessionRegistry>,
    cache: Arc<AuthKeyCache>,
}

/// A bound MTProto edge that admits connections and spawns an actor per
/// connection.
pub struct Server {
    ingress: Ingress,
    core: Arc<dyn Core>,
    options: Arc<ServerOptions>,
    budgets: Arc<Budgets>,
    registry: Arc<SessionRegistry>,
    cache: Arc<AuthKeyCache>,
}

impl Server {
    /// Creates a server over an already-bound `ingress`.
    #[must_use]
    pub fn new(ingress: Ingress, core: Arc<dyn Core>, options: ServerOptions) -> Self {
        let registry = Arc::new(SessionRegistry::new(
            options.clock.clone(),
            options.idle_timeout,
        ));
        let cache = Arc::new(AuthKeyCache::new());
        let budgets = Arc::new(Budgets::new(options.budget_limits));
        Self {
            ingress,
            core,
            options: Arc::new(options),
            budgets,
            registry,
            cache,
        }
    }

    /// Binds a listener on `addr` and creates a server.
    ///
    /// # Errors
    ///
    /// Returns [`IngressError`] when the address cannot be bound.
    pub async fn bind(
        addr: impl tokio::net::ToSocketAddrs,
        config: IngressConfig,
        core: Arc<dyn Core>,
        options: ServerOptions,
    ) -> Result<Self, IngressError> {
        let ingress = Ingress::bind(addr, config).await?;
        Ok(Self::new(ingress, core, options))
    }

    /// Returns the local address the listener is bound to.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] if the address cannot be resolved.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.ingress.local_addr()
    }

    /// Returns the shared budgets.
    #[must_use]
    pub fn budgets(&self) -> &Arc<Budgets> {
        &self.budgets
    }

    /// Returns the shared transient session registry.
    #[must_use]
    pub fn registry(&self) -> &Arc<SessionRegistry> {
        &self.registry
    }

    /// Returns the shared auth-key cache.
    #[must_use]
    pub fn cache(&self) -> &Arc<AuthKeyCache> {
        &self.cache
    }

    /// Accepts and admits the next connection, spawning its actor.
    ///
    /// Returns `Ok(None)` when the connection is shed by a budget (the socket is
    /// dropped); an admitted connection yields its actor's [`JoinHandle`].
    ///
    /// # Errors
    ///
    /// Returns [`IngressError`] when accepting or transport detection fails.
    pub async fn accept(&self) -> Result<Option<JoinHandle<()>>, IngressError> {
        let connection = self.ingress.accept().await?;
        let source = SourceKey::from_meta(connection.meta());
        let admission = match self.budgets.admit(source) {
            Ok(admission) => admission,
            Err(_) => return Ok(None),
        };
        let handshake = match self.budgets.begin_handshake() {
            Ok(handshake) => handshake,
            Err(_) => return Ok(None),
        };
        let context = ConnContext {
            core: self.core.clone(),
            options: self.options.clone(),
            budgets: self.budgets.clone(),
            registry: self.registry.clone(),
            cache: self.cache.clone(),
        };
        let meta = connection.meta().clone();
        Ok(Some(spawn_inner(
            connection,
            meta,
            context,
            Some(admission),
            Some(handshake),
        )))
    }

    /// Serves connections until the process is shut down.
    ///
    /// # Errors
    ///
    /// Returns [`IngressError`] when accepting fails.
    pub async fn serve(self) -> Result<(), IngressError> {
        loop {
            let _ = self.accept().await?;
        }
    }
}

/// Spawns a connection actor over an already-accepted stream.
///
/// This is the transport-agnostic entry point; [`Server::accept`] uses it with
/// the budgets already acquired.
pub fn spawn<S>(
    stream: S,
    meta: ConnectionMeta,
    core: Arc<dyn Core>,
    options: Arc<ServerOptions>,
    budgets: Arc<Budgets>,
    registry: Arc<SessionRegistry>,
    cache: Arc<AuthKeyCache>,
) -> JoinHandle<()>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let context = ConnContext {
        core,
        options,
        budgets,
        registry,
        cache,
    };
    spawn_inner(stream, meta, context, None, None)
}

fn spawn_inner<S>(
    stream: S,
    meta: ConnectionMeta,
    context: ConnContext,
    admission: Option<Admission>,
    handshake: Option<HandshakeGuard>,
) -> JoinHandle<()>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    tokio::spawn(run_connection(stream, meta, context, admission, handshake))
}

/// A codec boxed with an explicit `Send` bound so it can cross a task spawn.
fn build_codec(kind: CodecKind) -> Box<dyn Codec + Send> {
    match kind {
        CodecKind::Abridged => Box::new(Abridged),
        CodecKind::Intermediate => Box::new(Intermediate),
        CodecKind::PaddedIntermediate => Box::new(PaddedIntermediate::new()),
        CodecKind::Full => Box::new(Full::new()),
    }
}

/// Reads one transport frame asynchronously, then validates it with `codec`.
///
/// Only the length prefix is parsed here; the complete frame is replayed through
/// the codec from memory so its framing validation (full-framing sequence and
/// CRC, protocol errors) is shared with the rest of the stack.
async fn read_frame<R>(
    reader: &mut R,
    kind: CodecKind,
    codec: &mut (dyn Codec + Send),
) -> io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let (mut frame, total) = read_frame_prefix(reader, kind).await?;
    let prefix_len = frame.len();
    frame.resize(total, 0);
    reader.read_exact(&mut frame[prefix_len..]).await?;
    let mut cursor = io::Cursor::new(&frame);
    codec.read_message(&mut cursor).map_err(io::Error::other)
}

/// Reads and validates a frame's length prefix, returning it and the total size.
async fn read_frame_prefix<R>(reader: &mut R, kind: CodecKind) -> io::Result<(Vec<u8>, usize)>
where
    R: AsyncRead + Unpin,
{
    match kind {
        CodecKind::Abridged => {
            let mut first = [0u8; 1];
            reader.read_exact(&mut first).await?;
            let words = u32::from(first[0] & 0x7f);
            if words == 0 {
                return Err(invalid("zero-length abridged frame"));
            }
            if words < 0x7f {
                let total = 1 + (words as usize) * 4;
                check_total(total)?;
                Ok((vec![first[0]], total))
            } else {
                let mut extended = [0u8; 3];
                reader.read_exact(&mut extended).await?;
                let words = u32::from(extended[0])
                    | (u32::from(extended[1]) << 8)
                    | (u32::from(extended[2]) << 16);
                if words < 0x7f {
                    return Err(invalid("malformed extended abridged frame"));
                }
                let total = 4 + (words as usize) * 4;
                check_total(total)?;
                Ok((vec![first[0], extended[0], extended[1], extended[2]], total))
            }
        }
        CodecKind::Intermediate | CodecKind::PaddedIntermediate => {
            let mut length = [0u8; 4];
            reader.read_exact(&mut length).await?;
            let payload = u32::from_le_bytes(length) as usize;
            check_total(payload)?;
            Ok((length.to_vec(), 4 + payload))
        }
        CodecKind::Full => {
            let mut length = [0u8; 4];
            reader.read_exact(&mut length).await?;
            let total = u32::from_le_bytes(length) as usize;
            if !(3 * 4..=MAX_MESSAGE_SIZE).contains(&total) || !total.is_multiple_of(4) {
                return Err(invalid("invalid full-framing length"));
            }
            Ok((length.to_vec(), total))
        }
    }
}

fn check_total(total: usize) -> io::Result<()> {
    if total == 0 || total > MAX_MESSAGE_SIZE {
        Err(invalid("invalid frame length"))
    } else {
        Ok(())
    }
}

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

/// Sends one transport frame.
async fn write_frame<W>(
    out: &mut W,
    codec: &mut (dyn Codec + Send),
    payload: &[u8],
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let mut frame = Vec::with_capacity(payload.len() + 16);
    codec
        .write_message(&mut frame, payload)
        .map_err(io::Error::other)?;
    out.write_all(&frame).await?;
    out.flush().await
}

fn spawn_reader<S>(half: ReadHalf<S>, kind: CodecKind, tx: mpsc::Sender<Vec<u8>>) -> JoinHandle<()>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    tokio::spawn(async move {
        let mut codec = build_codec(kind);
        let mut half = half;
        while let Ok(frame) = read_frame(&mut half, kind, codec.as_mut()).await {
            if tx.send(frame).await.is_err() {
                break;
            }
        }
    })
}

fn spawn_writer<S>(
    half: WriteHalf<S>,
    kind: CodecKind,
    clock: Arc<dyn Clock + Send + Sync>,
    rx: mpsc::Receiver<WriterCommand>,
) -> JoinHandle<()>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    tokio::spawn(run_writer(half, kind, clock, rx))
}

/// The result of validating an inbound header, including any queued acks.
struct Validated {
    result: Result<Inbound, SessionError>,
    acks: Vec<i64>,
}

/// A command to the single writer task.
enum WriterCommand {
    /// Writes a raw, unencrypted frame (the key exchange and protocol errors).
    WriteRaw(Vec<u8>),
    /// Installs the negotiated authorization key.
    SetAuthKey(Box<AuthKey>),
    /// Adopts the session carried by the first decrypted frame.
    AdoptSession { session_id: i64, salt: i64 },
    /// Updates the current server salt.
    SetSalt(i64),
    /// Allocates, encrypts and writes a message, retaining content for resend.
    Send {
        ty: MessageType,
        class: MessageClass,
        body: Vec<u8>,
    },
    /// Re-writes retained outbound messages by their original identifiers.
    Resend(Vec<i64>),
    /// Drops acknowledged outbound content messages.
    Acknowledge(Vec<i64>),
    /// Validates an inbound header and drains the acknowledgement backlog.
    Validate {
        header: MessageHeader,
        reply: oneshot::Sender<Validated>,
    },
    /// Disconnects the session.
    Release(DisconnectReason),
}

/// A cloneable handle to the writer task.
#[derive(Clone)]
struct WriterHandle {
    tx: mpsc::Sender<WriterCommand>,
}

impl WriterHandle {
    async fn command(&self, command: WriterCommand) -> Result<(), ActorError> {
        self.tx.send(command).await.map_err(|_| ActorError::Closed)
    }

    async fn write_raw(&self, payload: &[u8]) -> Result<(), ActorError> {
        self.command(WriterCommand::WriteRaw(payload.to_vec()))
            .await
    }

    async fn set_auth_key(&self, key: AuthKey) -> Result<(), ActorError> {
        self.command(WriterCommand::SetAuthKey(Box::new(key))).await
    }

    async fn adopt_session(&self, session_id: i64, salt: i64) -> Result<(), ActorError> {
        self.command(WriterCommand::AdoptSession { session_id, salt })
            .await
    }

    async fn set_salt(&self, salt: i64) -> Result<(), ActorError> {
        self.command(WriterCommand::SetSalt(salt)).await
    }

    async fn send(
        &self,
        ty: MessageType,
        class: MessageClass,
        body: Vec<u8>,
    ) -> Result<(), ActorError> {
        self.command(WriterCommand::Send { ty, class, body }).await
    }

    async fn resend(&self, ids: Vec<i64>) -> Result<(), ActorError> {
        self.command(WriterCommand::Resend(ids)).await
    }

    async fn acknowledge(&self, ids: Vec<i64>) -> Result<(), ActorError> {
        self.command(WriterCommand::Acknowledge(ids)).await
    }

    async fn validate(&self, header: MessageHeader) -> Result<Validated, ActorError> {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(WriterCommand::Validate { header, reply })
            .await
            .map_err(|_| ActorError::Closed)?;
        response.await.map_err(|_| ActorError::Closed)
    }

    async fn release(&self, reason: DisconnectReason) -> Result<(), ActorError> {
        self.command(WriterCommand::Release(reason)).await
    }
}

/// Runs the single writer task: it owns the session machine and the write half.
async fn run_writer<W>(
    mut out: W,
    kind: CodecKind,
    clock: Arc<dyn Clock + Send + Sync>,
    mut rx: mpsc::Receiver<WriterCommand>,
) where
    W: AsyncWrite + Unpin,
{
    let mut codec = build_codec(kind);
    let mut key: Option<AuthKey> = None;
    let mut session: Option<Session> = None;
    let mut random = SystemRandom;

    while let Some(command) = rx.recv().await {
        let result = match command {
            WriterCommand::WriteRaw(payload) => {
                write_frame(&mut out, codec.as_mut(), &payload).await
            }
            WriterCommand::SetAuthKey(value) => {
                key = Some(*value);
                Ok(())
            }
            WriterCommand::AdoptSession { session_id, salt } => {
                if session.is_none() {
                    let mut adopted = Session::with_id(clock.clone(), session_id);
                    adopted.set_salt(salt);
                    session = Some(adopted);
                }
                Ok(())
            }
            WriterCommand::SetSalt(salt) => {
                if let Some(active) = session.as_mut() {
                    active.set_salt(salt);
                }
                Ok(())
            }
            WriterCommand::Send { ty, class, body } => {
                if let (Some(active_key), Some(active)) = (key.as_ref(), session.as_mut()) {
                    let sequence = active.allocate(ty, class);
                    let header = MessageHeader {
                        salt: active.salt(),
                        session_id: active.session_id(),
                        msg_id: sequence.msg_id,
                        seq_no: sequence.seq_no,
                    };
                    let frame =
                        rtd_crypto::encrypt(active_key, &header, &body, Side::Server, &mut random)
                            .encode();
                    if class == MessageClass::Content {
                        active.record_content(OutboundMessage::new(
                            sequence.msg_id,
                            sequence.seq_no,
                            body,
                        ));
                    }
                    write_frame(&mut out, codec.as_mut(), &frame).await
                } else {
                    Ok(())
                }
            }
            WriterCommand::Resend(ids) => {
                if let (Some(active_key), Some(active)) = (key.as_ref(), session.as_mut()) {
                    let mut result = Ok(());
                    for message in active.resend(&ids) {
                        let header = MessageHeader {
                            salt: active.salt(),
                            session_id: active.session_id(),
                            msg_id: message.msg_id,
                            seq_no: message.seq_no,
                        };
                        let frame = rtd_crypto::encrypt(
                            active_key,
                            &header,
                            &message.body,
                            Side::Server,
                            &mut random,
                        )
                        .encode();
                        result = write_frame(&mut out, codec.as_mut(), &frame).await;
                        if result.is_err() {
                            break;
                        }
                    }
                    result
                } else {
                    Ok(())
                }
            }
            WriterCommand::Acknowledge(ids) => {
                if let Some(active) = session.as_mut() {
                    active.acknowledge(&ids);
                }
                Ok(())
            }
            WriterCommand::Validate { header, reply } => {
                let result = match session.as_mut() {
                    Some(active) => active.accept_inbound(&header),
                    None => Err(SessionError::NotActive),
                };
                let acks = session
                    .as_mut()
                    .map(Session::drain_acks)
                    .unwrap_or_default();
                let _ = reply.send(Validated { result, acks });
                Ok(())
            }
            WriterCommand::Release(reason) => {
                if let Some(active) = session.as_mut() {
                    active.disconnect(reason);
                }
                Ok(())
            }
        };
        if result.is_err() {
            break;
        }
    }

    let _ = out.shutdown().await;
}

/// The outcome of the handshake phase.
struct HandshakeOutcome {
    auth_key: Option<AuthKey>,
    salt: i64,
    pending: Option<Vec<u8>>,
}

/// A [`rtd_exchange::Stream`] over the actor's async socket via channels.
struct ChannelStream {
    inbound: mpsc::UnboundedReceiver<Vec<u8>>,
    outbound: mpsc::UnboundedSender<Vec<u8>>,
}

impl ExchangeStream for ChannelStream {
    fn send(&mut self, frame: &[u8]) -> rtd_exchange::Result<()> {
        self.outbound
            .send(frame.to_vec())
            .map_err(|error| rtd_exchange::Error::Stream(error.to_string()))
    }

    fn recv(&mut self) -> rtd_exchange::Result<Vec<u8>> {
        self.inbound
            .blocking_recv()
            .ok_or_else(|| rtd_exchange::Error::Stream("connection closed".to_owned()))
    }
}

/// Drives the connection's handshake phase.
async fn run_handshake(
    inbound: &mut mpsc::Receiver<Vec<u8>>,
    writer: &WriterHandle,
    context: &ConnContext,
) -> Result<HandshakeOutcome, ActorError> {
    let first = match inbound.recv().await {
        Some(frame) => frame,
        None => {
            return Ok(HandshakeOutcome {
                auth_key: None,
                salt: 0,
                pending: None,
            });
        }
    };

    // A peer that already holds a key skips the exchange.
    if is_encrypted(&first) {
        return Ok(HandshakeOutcome {
            auth_key: None,
            salt: 0,
            pending: Some(first),
        });
    }

    let (request_tx, request_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (response_tx, mut response_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let exchange = context.options.exchange.clone();
    let clock = context.options.clock.clone();
    let mut handle = tokio::task::spawn_blocking(move || {
        let mut stream = ChannelStream {
            inbound: request_rx,
            outbound: response_tx,
        };
        let mut random = SystemRandom;
        exchange.exchange(&mut stream, &*clock, &mut random)
    });

    let _ = request_tx.send(first);
    let mut held: Option<Vec<u8>> = None;
    let mut pending: Option<Vec<u8>> = None;
    let mut finished: Option<rtd_exchange::Result<rtd_exchange::ServerExchangeResult>> = None;

    while finished.is_none() {
        tokio::select! {
            joined = &mut handle => {
                finished = Some(joined.expect("exchange thread panicked"));
            }
            frame = response_rx.recv() => {
                if let Some(frame) = frame {
                    if is_exchange_answer(&frame) {
                        held = Some(frame);
                    } else {
                        writer.write_raw(&frame).await?;
                    }
                }
            }
            maybe = inbound.recv() => {
                match maybe {
                    Some(frame) if is_encrypted(&frame) => {
                        pending = Some(frame);
                        break;
                    }
                    Some(frame) => {
                        let _ = request_tx.send(frame);
                    }
                    None => break,
                }
            }
        }
    }

    let outcome = if let Some(result) = finished {
        result
    } else {
        drop(request_tx);
        handle.await.expect("exchange thread panicked")
    };

    while let Ok(frame) = response_rx.try_recv() {
        if is_exchange_answer(&frame) {
            held = Some(frame);
        } else {
            writer.write_raw(&frame).await?;
        }
    }

    match outcome {
        Ok(result) => {
            if session::save_auth_key_before_confirm(
                &*context.core,
                &context.cache,
                &result.auth_key,
                context.options.dc,
            )
            .await
            .is_err()
            {
                // Fail closed: the exchange is not confirmed.
                return Err(ActorError::Core(crate::core::core_failure_error()));
            }
            if let Some(frame) = held.take() {
                writer.write_raw(&frame).await?;
            }
            Ok(HandshakeOutcome {
                auth_key: Some(result.auth_key),
                salt: result.server_salt,
                pending,
            })
        }
        Err(rtd_exchange::Error::UnexpectedEncrypted { .. }) => Ok(HandshakeOutcome {
            auth_key: None,
            salt: 0,
            pending,
        }),
        Err(error) => Err(ActorError::Exchange(error.to_string())),
    }
}

/// An actor error; every variant closes the connection.
#[derive(Debug)]
enum ActorError {
    /// The writer task is gone.
    Closed,
    /// The exchange failed.
    Exchange(String),
    /// The core failed.
    Core(RtdError),
}

impl std::fmt::Display for ActorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => f.write_str("the connection writer is closed"),
            Self::Exchange(reason) => write!(f, "key exchange failed: {reason}"),
            Self::Core(error) => write!(f, "core failure: {error}"),
        }
    }
}

impl std::error::Error for ActorError {}

/// The per-connection actor state after the handshake.
struct Actor {
    context: ConnContext,
    writer: WriterHandle,
    auth_key: Option<AuthKey>,
    auth_key_id: Option<AuthKeyId>,
    session_id: Option<i64>,
    salt: i64,
    created: bool,
    pending: Option<Vec<u8>>,
}

impl Actor {
    async fn run(&mut self, inbound: &mut mpsc::Receiver<Vec<u8>>) {
        loop {
            let frame = match self.pending.take() {
                Some(frame) => frame,
                None => match inbound.recv().await {
                    Some(frame) => frame,
                    None => break,
                },
            };
            match self.handle_frame(frame).await {
                Ok(true) => {}
                Ok(false) | Err(_) => break,
            }
        }
    }

    async fn handle_frame(&mut self, frame: Vec<u8>) -> Result<bool, ActorError> {
        let Some(key_id) = auth_key_id(&frame) else {
            return Ok(true);
        };
        if key_id == [0u8; 8] {
            return Ok(true);
        }
        let _frame_budget = match self.context.budgets.reserve_frame(frame.len()) {
            Ok(guard) => guard,
            Err(_) => return Ok(true),
        };

        let key = match self.resolve_key(key_id).await? {
            Some(key) => key,
            None => {
                self.writer
                    .write_raw(&protocol_error(CODE_AUTH_KEY_NOT_FOUND))
                    .await?;
                return Ok(true);
            }
        };

        let message = match EncryptedMessage::decode(&frame) {
            Ok(message) => message,
            Err(_) => return Ok(true),
        };
        let decrypted = match rtd_crypto::decrypt(&key, &message, Side::Client) {
            Ok(decrypted) => decrypted,
            Err(CryptoError::AuthKeyMismatch { .. }) => {
                self.writer
                    .write_raw(&protocol_error(CODE_AUTH_KEY_NOT_FOUND))
                    .await?;
                return Ok(true);
            }
            Err(_) => return Ok(true),
        };

        if self.session_id.is_none() {
            if self.salt == 0 {
                self.salt = decrypted.header.salt;
            }
            self.adopt(key, decrypted.header.session_id).await?;
        }
        if self.session_id != Some(decrypted.header.session_id) {
            return Ok(true);
        }

        let validated = self.writer.validate(decrypted.header).await?;
        match validated.result {
            Err(error) => self.on_validate_error(decrypted.header, error).await?,
            Ok(Inbound::Duplicate) => {}
            Ok(Inbound::Accepted) => {
                self.ensure_created(decrypted.header).await?;
                if !self.handle_body(decrypted.header, &decrypted.body).await? {
                    return Ok(false);
                }
            }
        }
        if !validated.acks.is_empty() {
            self.send_service(MessageClass::Service, encode_msgs_ack(&validated.acks))
                .await?;
        }
        if let (Some(auth_key_id), Some(session_id)) = (self.auth_key_id, self.session_id) {
            let _ = self
                .context
                .registry
                .touch(SessionKey::new(auth_key_id, SessionId(session_id)))
                .await;
        }
        Ok(true)
    }

    async fn resolve_key(&mut self, key_id: [u8; 8]) -> Result<Option<AuthKey>, ActorError> {
        if let Some(key) = self.auth_key
            && key.id == key_id
        {
            return Ok(Some(key));
        }
        match session::resolve_auth_key(&*self.context.core, &self.context.cache, AuthKeyId(key_id))
            .await
        {
            Ok(Some(key)) => {
                self.auth_key = Some(key);
                Ok(Some(key))
            }
            Ok(None) => Ok(None),
            // A core failure is indistinguishable from an unknown key here, and
            // both fail closed with an authorization-key-not-found answer.
            Err(_) => Ok(None),
        }
    }

    async fn adopt(&mut self, key: AuthKey, session_id: i64) -> Result<(), ActorError> {
        let key_id = AuthKeyId(key.id);
        self.auth_key_id = Some(key_id);
        self.session_id = Some(session_id);
        self.writer.set_auth_key(key).await?;
        self.writer.adopt_session(session_id, self.salt).await?;
        self.context
            .registry
            .register(SessionKey::new(key_id, SessionId(session_id)))
            .await;
        Ok(())
    }

    async fn ensure_created(&mut self, header: MessageHeader) -> Result<(), ActorError> {
        if self.created {
            return Ok(());
        }
        self.created = true;
        let unique_id =
            i64::from_le_bytes(rtd_util::random_bytes(8).try_into().expect("eight bytes"));
        let mut writer = Writer::new();
        NewSession::New_session_created {
            first_msg_id: header.msg_id,
            unique_id,
            server_salt: self.salt,
        }
        .encode(&mut writer);
        self.send_service(MessageClass::Service, writer.into_vec())
            .await
    }

    async fn handle_body(
        &mut self,
        header: MessageHeader,
        body: &[u8],
    ) -> Result<bool, ActorError> {
        match peek_id(body) {
            Ping::TYPE_ID => {
                if let Some(ping) = decode_body(body, Ping::decode) {
                    self.send_pong(header.msg_id, ping.ping_id).await?;
                }
            }
            Ping_delay_disconnect::TYPE_ID => {
                if let Some(ping) = decode_body(body, Ping_delay_disconnect::decode) {
                    self.send_pong(header.msg_id, ping.ping_id).await?;
                }
            }
            ID_MSGS_ACK => {
                if let Some(ack) = decode_body(body, MsgsAck::decode) {
                    let MsgsAck::Msgs_ack { msg_ids } = ack;
                    self.writer.acknowledge(msg_ids).await?;
                }
            }
            ID_GET_FUTURE_SALTS => {
                if let Some(request) = decode_body(body, Get_future_salts::decode) {
                    self.send_future_salts(header.msg_id, request.num).await?;
                }
            }
            ID_MSG_RESEND_REQ => {
                if let Some(request) = decode_body(body, MsgResendReq::decode) {
                    let MsgResendReq::Msg_resend_req { msg_ids } = request;
                    self.writer.resend(msg_ids).await?;
                }
            }
            ID_DESTROY_SESSION => {
                if let Some(request) = decode_body(body, Destroy_session::decode) {
                    self.send_destroy_answer(request.session_id).await?;
                    return Ok(false);
                }
            }
            ID_MSGS_STATE_REQ => {
                if let Some(request) = decode_body(body, MsgsStateReq::decode) {
                    let MsgsStateReq::Msgs_state_req { msg_ids } = request;
                    self.send_state_info(header.msg_id, msg_ids.len()).await?;
                }
            }
            ID_BAD_MSG_NOTIFICATION => {
                // The peer reported a bad message we sent; the connection is
                // already corrected by the salt/identifier handling elsewhere.
            }
            ID_BAD_SERVER_SALT => {
                if let Some(BadMsgNotification::Bad_server_salt {
                    new_server_salt, ..
                }) = decode_body(body, BadMsgNotification::decode)
                {
                    self.salt = new_server_salt;
                    self.writer.set_salt(new_server_salt).await?;
                }
            }
            _ => self.dispatch_content(header, body).await?,
        }
        Ok(true)
    }

    async fn on_validate_error(
        &self,
        header: MessageHeader,
        error: SessionError,
    ) -> Result<(), ActorError> {
        let code = match error {
            SessionError::MessageTooOld => 16,
            SessionError::MessageTooNew => 17,
            SessionError::OutsideReplayWindow => 32,
            _ => return Ok(()),
        };
        let mut writer = Writer::new();
        BadMsgNotification::Bad_msg_notification {
            bad_msg_id: header.msg_id,
            bad_msg_seqno: header.seq_no,
            error_code: code,
        }
        .encode(&mut writer);
        self.send_service(MessageClass::Service, writer.into_vec())
            .await
    }

    async fn send_pong(&self, msg_id: i64, ping_id: i64) -> Result<(), ActorError> {
        let mut writer = Writer::new();
        Pong { msg_id, ping_id }.encode(&mut writer);
        self.send_service(MessageClass::Service, writer.into_vec())
            .await
    }

    async fn send_future_salts(&self, req_msg_id: i64, num: i32) -> Result<(), ActorError> {
        let now = unix_seconds(&*self.context.options.clock);
        let count = num.clamp(1, 4) as usize;
        let salts = (0..count)
            .map(|index| FutureSalt::Future_salt {
                valid_since: now,
                valid_until: now.saturating_add(1800 * (index as i32 + 1)),
                salt: self.salt,
            })
            .collect();
        let mut writer = Writer::new();
        FutureSalts::Future_salts {
            req_msg_id,
            now,
            salts,
        }
        .encode(&mut writer);
        self.send_service(MessageClass::Service, writer.into_vec())
            .await
    }

    async fn send_state_info(&self, req_msg_id: i64, count: usize) -> Result<(), ActorError> {
        let mut writer = Writer::new();
        MsgsStateInfo::Msgs_state_info {
            req_msg_id,
            info: vec![1u8; count],
        }
        .encode(&mut writer);
        self.send_service(MessageClass::Service, writer.into_vec())
            .await
    }

    async fn send_destroy_answer(&self, requested: i64) -> Result<(), ActorError> {
        let session_id = self.session_id.unwrap_or(0);
        let answer = if requested == session_id {
            DestroySessionRes::Destroy_session_ok { session_id }
        } else {
            DestroySessionRes::Destroy_session_none {
                session_id: requested,
            }
        };
        let mut writer = Writer::new();
        answer.encode(&mut writer);
        self.send_service(MessageClass::Service, writer.into_vec())
            .await
    }

    async fn dispatch_content(
        &mut self,
        header: MessageHeader,
        body: &[u8],
    ) -> Result<(), ActorError> {
        let profile = self.context.options.layer;
        let admitted =
            match rtd_tlprofile::admit_default_with_limits(profile, body, Limits::default()) {
                Ok(admitted) => admitted,
                Err(error) => {
                    let mapped = crate::rpc::map_admit_error(&error);
                    return self.send_rpc_error(header.msg_id, &mapped).await;
                }
            };
        let call = AdmittedCall {
            session: SessionId(self.session_id.unwrap_or(0)),
            semantic: admitted.semantic,
            layer: Some(profile.layer()),
            wire_id: admitted.wire_id,
            request: body.to_vec(),
        };
        match self.context.core.dispatch(call).await {
            Ok(result) => {
                let payload = rtd_proto::RpcResult::new(header.msg_id, result.result).encode();
                self.send_content(payload).await
            }
            Err(error) => self.send_rpc_error(header.msg_id, &error).await,
        }
    }

    async fn send_rpc_error(&self, req_msg_id: i64, error: &RtdError) -> Result<(), ActorError> {
        let mut writer = Writer::new();
        RpcError::Rpc_error {
            error_code: error.code(),
            error_message: error.message().to_owned(),
        }
        .encode(&mut writer);
        let payload = rtd_proto::RpcResult::new(req_msg_id, writer.into_vec()).encode();
        self.send_content(payload).await
    }

    async fn send_service(&self, class: MessageClass, body: Vec<u8>) -> Result<(), ActorError> {
        self.writer.send(MessageType::FromServer, class, body).await
    }

    async fn send_content(&self, body: Vec<u8>) -> Result<(), ActorError> {
        self.writer
            .send(MessageType::ServerResponse, MessageClass::Content, body)
            .await
    }

    async fn finish(&mut self) {
        let _ = self.writer.release(DisconnectReason::TransportClosed).await;
        if let (Some(auth_key_id), Some(session_id)) = (self.auth_key_id, self.session_id) {
            self.context
                .registry
                .evict(SessionKey::new(auth_key_id, SessionId(session_id)))
                .await;
            let _ = self
                .context
                .core
                .report_session_offline(SessionId(session_id))
                .await;
        }
    }
}

/// The actor task: handshake, then the encrypted session loop.
async fn run_connection<S>(
    stream: S,
    meta: ConnectionMeta,
    context: ConnContext,
    admission: Option<Admission>,
    handshake: Option<HandshakeGuard>,
) where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let _admission = admission;
    let (read_half, write_half) = tokio::io::split(stream);
    let (inbound_tx, mut inbound_rx) = mpsc::channel::<Vec<u8>>(INBOUND_QUEUE);
    let (writer_tx, writer_rx) = mpsc::channel::<WriterCommand>(WRITER_QUEUE);
    let writer = WriterHandle { tx: writer_tx };

    let reader = spawn_reader(read_half, meta.codec, inbound_tx);
    let writer_task = spawn_writer(
        write_half,
        meta.codec,
        context.options.clock.clone(),
        writer_rx,
    );

    let mut handshake = handshake;
    let outcome = run_handshake(&mut inbound_rx, &writer, &context).await;
    drop(handshake.take());

    match outcome {
        Ok(outcome) => {
            let mut actor = Actor {
                auth_key_id: outcome.auth_key.as_ref().map(|key| AuthKeyId(key.id)),
                auth_key: outcome.auth_key,
                session_id: None,
                salt: outcome.salt,
                created: false,
                pending: outcome.pending,
                writer,
                context,
            };
            actor.run(&mut inbound_rx).await;
            actor.finish().await;
            drop(actor);
        }
        Err(_) => {
            drop(writer);
        }
    }

    drop(inbound_rx);
    reader.abort();
    let _ = reader.await;
    let _ = writer_task.await;
}

/// Returns `true` when `frame` is an encrypted frame (non-zero auth-key id).
fn is_encrypted(frame: &[u8]) -> bool {
    auth_key_id(frame).is_some_and(|id| id != [0u8; 8])
}

/// Returns `true` when `frame` is a `set_client_DH_params_answer`.
fn is_exchange_answer(frame: &[u8]) -> bool {
    let Ok(message) = UnencryptedMessage::decode(frame) else {
        return false;
    };
    let Some(bytes) = message.data.get(..4) else {
        return false;
    };
    let Ok(bytes) = <[u8; 4]>::try_from(bytes) else {
        return false;
    };
    matches!(
        u32::from_le_bytes(bytes),
        ID_DH_GEN_OK | ID_DH_GEN_RETRY | ID_DH_GEN_FAIL
    )
}

/// Encodes a transport protocol error frame payload.
fn protocol_error(code: i32) -> Vec<u8> {
    (-code).to_le_bytes().to_vec()
}

/// Returns the constructor identifier at the start of `body`, or zero.
fn peek_id(body: &[u8]) -> u32 {
    body.get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .unwrap_or(0)
}

/// Decodes a TL body with default limits.
fn decode_body<T>(
    body: &[u8],
    decode: impl FnOnce(&mut MtDecoder<'_>) -> rtd_bin::Result<T>,
) -> Option<T> {
    let mut decoder = MtDecoder::new(body, MtDecodeLimits::default());
    decode(&mut decoder).ok()
}

/// Encodes a `msgs_ack` body.
fn encode_msgs_ack(ids: &[i64]) -> Vec<u8> {
    let mut writer = Writer::new();
    MsgsAck::Msgs_ack {
        msg_ids: ids.to_vec(),
    }
    .encode(&mut writer);
    writer.into_vec()
}

/// Returns the current Unix time in seconds.
fn unix_seconds(clock: &dyn Clock) -> i32 {
    clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| i32::try_from(since.as_secs()).unwrap_or(i32::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use rtd_bin::Int128;
    use rtd_crypto::Key;
    use rtd_tlprofile::CANONICAL_LAYER;
    use rtd_util::SystemClock;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

    use super::*;
    use crate::core::{
        AuthKeyRecord, CORE_PROTOCOL_VERSION, CallResult, CoreFuture, CoreInfo, LayerEvidence,
        core_failure_error,
    };

    fn test_key() -> AuthKey {
        let mut bytes = [0u8; 256];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = index as u8;
        }
        AuthKey::new(Key::from_array(bytes))
    }

    fn record_for(key: &AuthKey) -> AuthKeyRecord {
        AuthKeyRecord {
            id: AuthKeyId(key.id),
            dc: 2,
            key: key.value.as_bytes().to_vec(),
        }
    }

    /// An in-memory core whose key store can be inspected and whose save can be
    /// made to fail.
    struct FakeCore {
        keys: Mutex<HashMap<AuthKeyId, AuthKeyRecord>>,
        saved: Mutex<Vec<AuthKeyId>>,
        fail_save: AtomicBool,
    }

    impl FakeCore {
        fn new() -> Self {
            Self {
                keys: Mutex::new(HashMap::new()),
                saved: Mutex::new(Vec::new()),
                fail_save: AtomicBool::new(false),
            }
        }

        fn with_key(key: &AuthKey) -> Self {
            let core = Self::new();
            core.keys
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(AuthKeyId(key.id), record_for(key));
            core
        }

        fn has_key(&self, id: AuthKeyId) -> bool {
            self.keys
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains_key(&id)
        }
    }

    impl Core for FakeCore {
        fn info(&self) -> CoreFuture<'_, Result<CoreInfo, RtdError>> {
            Box::pin(async move {
                Ok(CoreInfo {
                    protocol_version: CORE_PROTOCOL_VERSION,
                    dc: 2,
                    capabilities: Vec::new(),
                })
            })
        }

        fn dispatch(&self, _call: AdmittedCall) -> CoreFuture<'_, Result<CallResult, RtdError>> {
            Box::pin(async move { Err(core_failure_error()) })
        }

        fn save_auth_key(&self, key: AuthKeyRecord) -> CoreFuture<'_, Result<(), RtdError>> {
            let fail = self.fail_save.load(Ordering::Relaxed);
            Box::pin(async move {
                if fail {
                    return Err(core_failure_error());
                }
                self.saved
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(key.id);
                self.keys
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
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
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(&id)
                    .cloned();
                Ok(key)
            })
        }

        fn resolve_layer_evidence(
            &self,
            _session: SessionId,
        ) -> CoreFuture<'_, Result<Option<i32>, RtdError>> {
            Box::pin(async move { Ok(None) })
        }

        fn advance_layer_evidence(
            &self,
            _session: SessionId,
            _layer: i32,
        ) -> CoreFuture<'_, Result<(), RtdError>> {
            Box::pin(async move { Ok(()) })
        }

        fn publish_layer_evidence(
            &self,
            _evidence: LayerEvidence,
        ) -> CoreFuture<'_, Result<(), RtdError>> {
            Box::pin(async move { Ok(()) })
        }

        fn report_session_offline(
            &self,
            _session: SessionId,
        ) -> CoreFuture<'_, Result<(), RtdError>> {
            Box::pin(async move { Ok(()) })
        }
    }

    /// An exchange that must never be called (used when a key is pre-supplied).
    fn never_exchange() -> Arc<dyn Exchange> {
        Arc::new(
            |_: &mut dyn ExchangeStream,
             _: &dyn Clock,
             _: &mut dyn rtd_crypto::RandomSource|
             -> rtd_exchange::Result<rtd_exchange::ServerExchangeResult> {
                Err(rtd_exchange::Error::Decode(
                    "exchange must not run".to_owned(),
                ))
            },
        )
    }

    /// A deterministic exchange: reads one plaintext frame, answers with a
    /// `dh_gen_ok`, and returns a fixed key.
    fn fixed_exchange(key: AuthKey, salt: i64) -> Arc<dyn Exchange> {
        Arc::new(
            move |stream: &mut dyn ExchangeStream,
                  _: &dyn Clock,
                  _: &mut dyn rtd_crypto::RandomSource|
                  -> rtd_exchange::Result<rtd_exchange::ServerExchangeResult> {
                let _request = stream.recv()?;
                let mut writer = Writer::new();
                rtd_mt::Set_client_DH_params_answer::Dh_gen_ok {
                    nonce: Int128::from_array([1u8; 16]),
                    server_nonce: Int128::from_array([2u8; 16]),
                    new_nonce_hash1: Int128::from_array([3u8; 16]),
                }
                .encode(&mut writer);
                let frame = UnencryptedMessage::new(999, writer.into_vec())
                    .encode()
                    .map_err(|error| rtd_exchange::Error::Decode(error.to_string()))?;
                stream.send(&frame)?;
                Ok(rtd_exchange::ServerExchangeResult {
                    auth_key: key,
                    server_salt: salt,
                })
            },
        )
    }

    fn options(exchange: Arc<dyn Exchange>, limits: BudgetLimits) -> ServerOptions {
        ServerOptions {
            dc: 2,
            clock: Arc::new(SystemClock),
            exchange,
            layer: Profile::new(CANONICAL_LAYER),
            idle_timeout: Duration::from_secs(300),
            budget_limits: limits,
        }
    }

    struct Harness {
        client: DuplexStream,
        budgets: Arc<Budgets>,
        handle: JoinHandle<()>,
    }

    fn start(
        core: Arc<FakeCore>,
        exchange: Arc<dyn Exchange>,
        limits: BudgetLimits,
        source: SourceKey,
    ) -> Harness {
        let budgets = Arc::new(Budgets::new(limits));
        let (client, server) = tokio::io::duplex(1 << 16);
        let meta = ConnectionMeta {
            codec: CodecKind::Intermediate,
            mode: TransportMode::Plain,
            dc_id: None,
            proxy_user: None,
        };
        let options = Arc::new(options(exchange, limits));
        let context = ConnContext {
            core: core.clone(),
            options: options.clone(),
            budgets: budgets.clone(),
            registry: Arc::new(SessionRegistry::new(
                options.clock.clone(),
                options.idle_timeout,
            )),
            cache: Arc::new(AuthKeyCache::new()),
        };
        let admission = budgets.admit(source).expect("admit");
        let handshake = budgets.begin_handshake().expect("handshake");
        let handle = tokio::spawn(run_connection(
            server,
            meta,
            context,
            Some(admission),
            Some(handshake),
        ));
        Harness {
            client,
            budgets,
            handle,
        }
    }

    fn client_frame(
        session: &mut Session,
        key: &AuthKey,
        salt: i64,
        class: MessageClass,
        body: &[u8],
    ) -> Vec<u8> {
        let sequence = session.allocate(MessageType::FromClient, class);
        let header = MessageHeader {
            salt,
            session_id: session.session_id(),
            msg_id: sequence.msg_id,
            seq_no: sequence.seq_no,
        };
        let mut random = SystemRandom;
        rtd_crypto::encrypt(key, &header, body, Side::Client, &mut random).encode()
    }

    async fn send_wire(client: &mut DuplexStream, payload: &[u8]) {
        let mut frame = Vec::new();
        Intermediate
            .write_message(&mut frame, payload)
            .expect("frame");
        client.write_all(&frame).await.expect("write");
        client.flush().await.expect("flush");
    }

    async fn read_wire(client: &mut DuplexStream) -> Vec<u8> {
        let mut length = [0u8; 4];
        client.read_exact(&mut length).await.expect("length");
        let size = u32::from_le_bytes(length) as usize;
        let mut payload = vec![0u8; size];
        client.read_exact(&mut payload).await.expect("payload");
        payload
    }

    fn server_message(key: &AuthKey, frame: &[u8]) -> (MessageHeader, Vec<u8>) {
        let message = EncryptedMessage::decode(frame).expect("encrypted frame");
        let decrypted = rtd_crypto::decrypt(key, &message, Side::Server).expect("decrypt");
        (decrypted.header, decrypted.body)
    }

    fn encode_ping(ping_id: i64) -> Vec<u8> {
        let mut writer = Writer::new();
        Ping { ping_id }.encode(&mut writer);
        writer.into_vec()
    }

    fn encode_get_config() -> Vec<u8> {
        let mut writer = Writer::new();
        rtd_tg::Method::HelpGetConfig(Box::new(rtd_tg::HelpGetConfig)).encode(&mut writer);
        writer.into_vec()
    }

    #[tokio::test]
    async fn service_ping_is_answered_by_pong() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 42);
        let frame = client_frame(
            &mut session,
            &key,
            0x1122_3344_5566_7788,
            MessageClass::Service,
            &encode_ping(7),
        );
        send_wire(&mut harness.client, &frame).await;

        // The first message creates the session, the second answers the ping.
        let created = read_wire(&mut harness.client).await;
        let (_, created_body) = server_message(&key, &created);
        assert_eq!(
            peek_id(&created_body),
            NewSession::New_session_created {
                first_msg_id: 0,
                unique_id: 0,
                server_salt: 0,
            }
            .type_id()
        );

        let pong = read_wire(&mut harness.client).await;
        let (_, pong_body) = server_message(&key, &pong);
        let Pong { ping_id, .. } =
            Pong::decode(&mut MtDecoder::new(&pong_body, MtDecodeLimits::default())).expect("pong");
        assert_eq!(ping_id, 7);

        harness.handle.abort();
    }

    #[tokio::test]
    async fn service_ping_delay_disconnect_is_answered_by_pong() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 43);
        let mut writer = Writer::new();
        Ping_delay_disconnect {
            ping_id: 9,
            disconnect_delay: 60,
        }
        .encode(&mut writer);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &writer.into_vec(),
        );
        send_wire(&mut harness.client, &frame).await;

        let _created = read_wire(&mut harness.client).await;
        let pong = read_wire(&mut harness.client).await;
        let (_, body) = server_message(&key, &pong);
        let Pong { ping_id, .. } =
            Pong::decode(&mut MtDecoder::new(&body, MtDecodeLimits::default())).expect("pong");
        assert_eq!(ping_id, 9);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn service_get_future_salts_is_answered() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 44);
        let mut writer = Writer::new();
        Get_future_salts { num: 2 }.encode(&mut writer);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &writer.into_vec(),
        );
        send_wire(&mut harness.client, &frame).await;

        let _created = read_wire(&mut harness.client).await;
        let response = read_wire(&mut harness.client).await;
        let (_, body) = server_message(&key, &response);
        let FutureSalts::Future_salts { salts, .. } =
            FutureSalts::decode(&mut MtDecoder::new(&body, MtDecodeLimits::default()))
                .expect("future_salts");
        assert_eq!(salts.len(), 2);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn service_msgs_ack_is_handled() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 45);
        // A content request produces a retained content result and an msgs_ack.
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Content,
            &encode_get_config(),
        );
        send_wire(&mut harness.client, &frame).await;
        let _created = read_wire(&mut harness.client).await;
        let result = read_wire(&mut harness.client).await;
        let (header, result_body) = server_message(&key, &result);
        assert_eq!(peek_id(&result_body), rtd_proto::RPC_RESULT_TYPE_ID);
        let ack = read_wire(&mut harness.client).await;
        assert_eq!(peek_id(&server_message(&key, &ack).1), ID_MSGS_ACK);

        // Acknowledging the result drops it from the resend queue.
        let mut writer = Writer::new();
        MsgsAck::Msgs_ack {
            msg_ids: vec![header.msg_id],
        }
        .encode(&mut writer);
        let acks = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &writer.into_vec(),
        );
        send_wire(&mut harness.client, &acks).await;
        harness.handle.abort();
    }

    #[tokio::test]
    async fn service_msg_resend_req_resends_pending() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 46);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Content,
            &encode_get_config(),
        );
        send_wire(&mut harness.client, &frame).await;
        let _created = read_wire(&mut harness.client).await;
        let result = read_wire(&mut harness.client).await;
        let (header, _) = server_message(&key, &result);
        let _ack = read_wire(&mut harness.client).await;

        let mut writer = Writer::new();
        MsgResendReq::Msg_resend_req {
            msg_ids: vec![header.msg_id],
        }
        .encode(&mut writer);
        let request = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &writer.into_vec(),
        );
        send_wire(&mut harness.client, &request).await;
        let resent = read_wire(&mut harness.client).await;
        let (resent_header, resent_body) = server_message(&key, &resent);
        assert_eq!(resent_header.msg_id, header.msg_id);
        assert_eq!(peek_id(&resent_body), rtd_proto::RPC_RESULT_TYPE_ID);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn service_msgs_state_req_is_answered() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 47);
        let mut writer = Writer::new();
        MsgsStateReq::Msgs_state_req {
            msg_ids: vec![10, 11, 12],
        }
        .encode(&mut writer);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &writer.into_vec(),
        );
        send_wire(&mut harness.client, &frame).await;

        let _created = read_wire(&mut harness.client).await;
        let response = read_wire(&mut harness.client).await;
        let (_, body) = server_message(&key, &response);
        let MsgsStateInfo::Msgs_state_info { info, .. } =
            MsgsStateInfo::decode(&mut MtDecoder::new(&body, MtDecodeLimits::default()))
                .expect("msgs_state_info");
        assert_eq!(info.len(), 3);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn service_destroy_session_answers_and_closes() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 48);
        let mut writer = Writer::new();
        Destroy_session { session_id: 48 }.encode(&mut writer);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &writer.into_vec(),
        );
        send_wire(&mut harness.client, &frame).await;

        let _created = read_wire(&mut harness.client).await;
        let response = read_wire(&mut harness.client).await;
        let (_, body) = server_message(&key, &response);
        match DestroySessionRes::decode(&mut MtDecoder::new(&body, MtDecodeLimits::default()))
            .expect("destroy_session_ok")
        {
            DestroySessionRes::Destroy_session_ok { session_id } => assert_eq!(session_id, 48),
            DestroySessionRes::Destroy_session_none { session_id } => {
                panic!("expected destroy_session_ok, got none for {session_id}")
            }
        }
        harness.handle.await.expect("actor closes");
    }

    #[tokio::test]
    async fn service_bad_server_salt_updates_the_salt() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 49);
        let mut writer = Writer::new();
        BadMsgNotification::Bad_server_salt {
            bad_msg_id: 1,
            bad_msg_seqno: 1,
            error_code: 48,
            new_server_salt: 0x77,
        }
        .encode(&mut writer);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &writer.into_vec(),
        );
        send_wire(&mut harness.client, &frame).await;
        let _created = read_wire(&mut harness.client).await;

        // A follow-up message is emitted with the updated salt.
        let ping = client_frame(
            &mut session,
            &key,
            0x77,
            MessageClass::Service,
            &encode_ping(3),
        );
        send_wire(&mut harness.client, &ping).await;
        let pong = read_wire(&mut harness.client).await;
        let (header, _) = server_message(&key, &pong);
        assert_eq!(header.salt, 0x77);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn service_bad_msg_notification_is_tolerated() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 53);
        let mut writer = Writer::new();
        BadMsgNotification::Bad_msg_notification {
            bad_msg_id: 1,
            bad_msg_seqno: 1,
            error_code: 16,
        }
        .encode(&mut writer);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &writer.into_vec(),
        );
        send_wire(&mut harness.client, &frame).await;
        let _created = read_wire(&mut harness.client).await;

        // The connection remains usable after the notification.
        let ping = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &encode_ping(6),
        );
        send_wire(&mut harness.client, &ping).await;
        let pong = read_wire(&mut harness.client).await;
        let (_, body) = server_message(&key, &pong);
        let Pong { ping_id, .. } =
            Pong::decode(&mut MtDecoder::new(&body, MtDecodeLimits::default())).expect("pong");
        assert_eq!(ping_id, 6);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn read_frame_round_trips_every_framing() {
        for kind in [
            CodecKind::Abridged,
            CodecKind::Intermediate,
            CodecKind::PaddedIntermediate,
            CodecKind::Full,
        ] {
            let (mut client, mut server) = tokio::io::duplex(8192);
            let payload = vec![7u8; 16];
            let mut encode_codec = build_codec(kind);
            let mut wire = Vec::new();
            encode_codec
                .write_message(&mut wire, &payload)
                .expect("encode");
            client.write_all(&wire).await.expect("write");
            client.flush().await.expect("flush");

            let mut decode_codec = build_codec(kind);
            let decoded = read_frame(&mut server, kind, decode_codec.as_mut())
                .await
                .expect("decode");
            assert_eq!(decoded, payload, "framing {kind:?}");
        }
    }

    #[tokio::test]
    async fn unknown_key_is_answered_with_auth_key_not_found() {
        let core = Arc::new(FakeCore::new());
        let mut harness = start(
            core,
            never_exchange(),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let key = test_key();
        let mut session = Session::with_id(Arc::new(SystemClock), 50);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &encode_ping(1),
        );
        send_wire(&mut harness.client, &frame).await;

        let response = read_wire(&mut harness.client).await;
        assert_eq!(response.len(), 4);
        let code = i32::from_le_bytes(response.try_into().expect("four bytes"));
        assert_eq!(code.wrapping_neg(), CODE_AUTH_KEY_NOT_FOUND);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn exchange_then_encrypted_frame_saves_key_before_confirming() {
        let key = test_key();
        let salt = 0x0b0a_0908_0706_0504;
        let core = Arc::new(FakeCore::new());
        let mut harness = start(
            core.clone(),
            fixed_exchange(key, salt),
            BudgetLimits::default(),
            SourceKey(1),
        );

        // A plaintext request triggers the (fake) exchange.
        let request = UnencryptedMessage::new(7, vec![0xde, 0xad, 0xbe, 0xef])
            .encode()
            .expect("plaintext");
        send_wire(&mut harness.client, &request).await;

        // The confirmation is only emitted after the key was saved.
        let confirmation = read_wire(&mut harness.client).await;
        let message = UnencryptedMessage::decode(&confirmation).expect("plaintext");
        assert_eq!(peek_id(&message.data), ID_DH_GEN_OK);
        assert!(core.has_key(AuthKeyId(key.id)), "key saved before confirm");
        assert_eq!(
            core.saved
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_slice(),
            &[AuthKeyId(key.id)]
        );

        // The client can now use an encrypted ping.
        let mut session = Session::with_id(Arc::new(SystemClock), 51);
        let frame = client_frame(
            &mut session,
            &key,
            salt,
            MessageClass::Service,
            &encode_ping(4),
        );
        send_wire(&mut harness.client, &frame).await;
        let _created = read_wire(&mut harness.client).await;
        let pong = read_wire(&mut harness.client).await;
        let (_, body) = server_message(&key, &pong);
        let Pong { ping_id, .. } =
            Pong::decode(&mut MtDecoder::new(&body, MtDecodeLimits::default())).expect("pong");
        assert_eq!(ping_id, 4);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn exchange_save_failure_fails_closed() {
        let key = test_key();
        let core = Arc::new(FakeCore::new());
        core.fail_save.store(true, Ordering::Relaxed);
        let mut harness = start(
            core.clone(),
            fixed_exchange(key, 0x1234),
            BudgetLimits::default(),
            SourceKey(1),
        );

        let request = UnencryptedMessage::new(7, vec![1, 2, 3, 4])
            .encode()
            .expect("plaintext");
        send_wire(&mut harness.client, &request).await;

        // The actor closes without writing the confirmation.
        harness.handle.await.expect("actor closes");
        assert!(!core.has_key(AuthKeyId(key.id)));
    }

    #[tokio::test]
    async fn writer_assigns_monotonic_ids_under_interleaved_writes() {
        let key = test_key();
        let (server_io, mut client_io) = tokio::io::duplex(1 << 16);
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock);
        let (tx, rx) = mpsc::channel(64);
        let writer_task = tokio::spawn(run_writer(server_io, CodecKind::Intermediate, clock, rx));
        let handle = WriterHandle { tx };
        handle.set_auth_key(key).await.expect("set key");
        handle.adopt_session(42, 0x55).await.expect("adopt");

        let bodies: Vec<Vec<u8>> = (0..4).map(|index| vec![index as u8; 4]).collect();
        for body in &bodies {
            handle
                .send(
                    MessageType::ServerResponse,
                    MessageClass::Content,
                    body.clone(),
                )
                .await
                .expect("send");
        }
        drop(handle);

        let mut ids = Vec::new();
        let mut seqs = Vec::new();
        let mut received = Vec::new();
        for _ in 0..4 {
            let frame = read_wire(&mut client_io).await;
            let (header, body) = server_message(&key, &frame);
            ids.push(header.msg_id);
            seqs.push(header.seq_no);
            received.push(body);
        }
        assert!(ids.windows(2).all(|window| window[0] < window[1]));
        assert_eq!(seqs, vec![1, 3, 5, 7]);
        assert_eq!(received, bodies);
        writer_task.await.expect("writer exits");
    }

    #[test]
    fn budgets_refuse_and_release() {
        let budgets = Arc::new(Budgets::new(BudgetLimits {
            max_connections: 1,
            max_per_source: 1,
            max_handshakes: 1,
            max_frame_bytes: 10,
        }));

        let first = budgets.admit(SourceKey(1)).expect("first");
        assert_eq!(
            budgets.admit(SourceKey(2)).expect_err("total"),
            Refusal::Connections
        );

        let handshake = budgets.begin_handshake().expect("handshake");
        assert_eq!(
            budgets.begin_handshake().expect_err("handshakes"),
            Refusal::Handshakes
        );
        drop(handshake);
        assert!(budgets.begin_handshake().is_ok());

        let frame = budgets.reserve_frame(6).expect("frame");
        assert_eq!(
            budgets.reserve_frame(5).expect_err("frames"),
            Refusal::FrameBytes
        );
        drop(frame);
        assert!(budgets.reserve_frame(5).is_ok());

        drop(first);
        // The total budget was released with the admission.
        let second = budgets.admit(SourceKey(1)).expect("after release");
        assert_eq!(budgets.connections(), 1);
        drop(second);
        assert_eq!(budgets.connections(), 0);

        // The per-source budget is independent of the total budget.
        let per_source = Arc::new(Budgets::new(BudgetLimits {
            max_connections: 2,
            max_per_source: 1,
            max_handshakes: 1,
            max_frame_bytes: 10,
        }));
        let a = per_source.admit(SourceKey(1)).expect("source a");
        assert_eq!(
            per_source.admit(SourceKey(1)).expect_err("per source"),
            Refusal::PerSource
        );
        let b = per_source.admit(SourceKey(2)).expect("source b");
        drop(a);
        drop(b);
        assert_eq!(per_source.connections(), 0);
    }

    #[tokio::test]
    async fn budget_refusal_leaves_admitted_session_running() {
        let key = test_key();
        let core = Arc::new(FakeCore::with_key(&key));
        let limits = BudgetLimits {
            max_connections: 1,
            max_per_source: 8,
            max_handshakes: 8,
            max_frame_bytes: 1 << 20,
        };
        let mut harness = start(core, never_exchange(), limits, SourceKey(1));

        // A second connection is refused, but the admitted one keeps working.
        assert_eq!(
            harness.budgets.admit(SourceKey(2)).expect_err("refused"),
            Refusal::Connections
        );

        let mut session = Session::with_id(Arc::new(SystemClock), 52);
        let frame = client_frame(
            &mut session,
            &key,
            0,
            MessageClass::Service,
            &encode_ping(5),
        );
        send_wire(&mut harness.client, &frame).await;
        let _created = read_wire(&mut harness.client).await;
        let pong = read_wire(&mut harness.client).await;
        let (_, body) = server_message(&key, &pong);
        let Pong { ping_id, .. } =
            Pong::decode(&mut MtDecoder::new(&body, MtDecodeLimits::default())).expect("pong");
        assert_eq!(ping_id, 5);
        harness.handle.abort();
    }

    #[tokio::test]
    async fn server_binds_and_admits_over_tcp() {
        let core: Arc<dyn Core> = Arc::new(FakeCore::new());
        let options = options(never_exchange(), BudgetLimits::default());
        let server = Server::bind("127.0.0.1:0", IngressConfig::default(), core, options)
            .await
            .expect("bind");
        let addr = server.local_addr().expect("addr");

        let client = tokio::spawn(async move {
            let mut socket = tokio::net::TcpStream::connect(addr).await.expect("connect");
            socket
                .write_all(&rtd_proto::codec::INTERMEDIATE_HEADER)
                .await
                .expect("header");
            socket.flush().await.expect("flush");
        });

        let handle = server
            .accept()
            .await
            .expect("accept")
            .expect("admitted connection");
        client.await.expect("client task");
        handle.await.expect("actor");
    }

    #[tokio::test]
    async fn server_sheds_after_budget_exhaustion() {
        let key = test_key();
        let core: Arc<dyn Core> = Arc::new(FakeCore::with_key(&key));
        let limits = BudgetLimits {
            max_connections: 1,
            max_per_source: 8,
            max_handshakes: 8,
            max_frame_bytes: 1 << 20,
        };
        let server = Server::bind(
            "127.0.0.1:0",
            IngressConfig::default(),
            core,
            options(never_exchange(), limits),
        )
        .await
        .expect("bind");
        let addr = server.local_addr().expect("addr");

        let (first_tx, first_rx) = tokio::sync::oneshot::channel();
        let first = tokio::spawn(async move {
            let mut socket = tokio::net::TcpStream::connect(addr).await.expect("connect");
            socket
                .write_all(&rtd_proto::codec::INTERMEDIATE_HEADER)
                .await
                .expect("header");
            socket.flush().await.expect("flush");
            let _ = first_rx.await;
        });
        let admitted = server.accept().await.expect("accept").expect("admitted");

        let (second_tx, mut second_rx) = tokio::sync::mpsc::channel(1);
        let second = tokio::spawn(async move {
            let mut socket = tokio::net::TcpStream::connect(addr)
                .await
                .expect("connect2");
            socket
                .write_all(&rtd_proto::codec::INTERMEDIATE_HEADER)
                .await
                .expect("header2");
            socket.flush().await.expect("flush2");
            let _ = second_rx.recv().await;
        });
        let shed = server.accept().await.expect("accept2");
        assert!(shed.is_none());
        let _ = second_tx.send(()).await;
        let _ = first_tx.send(());
        first.await.expect("first");
        second.await.expect("second");
        admitted.abort();
    }

    #[test]
    fn source_key_is_stable_and_distinguishes_metadata() {
        let meta = ConnectionMeta {
            codec: CodecKind::Intermediate,
            mode: TransportMode::Plain,
            dc_id: Some(2),
            proxy_user: None,
        };
        assert_eq!(
            SourceKey::from_meta(&meta),
            SourceKey::from_meta(&meta.clone())
        );
        let mut other = meta.clone();
        other.dc_id = Some(3);
        assert_ne!(SourceKey::from_meta(&meta), SourceKey::from_meta(&other));
    }

    #[test]
    fn protocol_error_payload_negates_the_code() {
        assert_eq!(
            protocol_error(CODE_AUTH_KEY_NOT_FOUND),
            (-CODE_AUTH_KEY_NOT_FOUND).to_le_bytes().to_vec()
        );
    }
}
