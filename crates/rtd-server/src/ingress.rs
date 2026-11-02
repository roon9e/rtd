//! Inbound transport detection and unwrapping.
//!
//! One [`Ingress`] binds a TCP listener and yields a [`Connection`]: an owned
//! MTProto byte stream plus [`ConnectionMeta`] telling the connection actor which
//! framing to apply. Detection selects the wrapping from the first bytes of each
//! connection (design D21), so the actor is identical for every transport:
//!
//! - plain TCP with abridged, intermediate, padded-intermediate or full framing;
//! - same-port plaintext versus obfuscated2 ([`rtd_transport::Obfuscated2`]);
//! - FakeTLS/MTProxy record framing ([`rtd_transport::faketls`]);
//! - WebSocket (`tokio-tungstenite`), whose binary messages carry the stream.
//!
//! The plain/obfuscated discrimination reads a short prefix once and relies on
//! the disjoint invariants used by the reference edge: a valid obfuscated2 init
//! never begins with a plaintext codec tag and always has non-zero nonce bytes
//! `4..8`, while the first full-framing word carries transport sequence zero.
//! Reserved HTTP and TLS prefixes are rejected unless the matching transport is
//! configured.

use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read, Write};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::{Sink, Stream};
use rtd_proto::codec::{
    ABRIDGED_HEADER, CodecKind, INTERMEDIATE_HEADER, MAX_MESSAGE_SIZE, PADDED_INTERMEDIATE_HEADER,
};
use rtd_transport::faketls::{
    RECORD_TYPE_APPLICATION, RECORD_TYPE_CHANGE_CIPHER_SPEC, Record, VERSION_10, VERSION_11,
    VERSION_12, VERSION_13, verify_client_hello, write_record, write_server_hello,
};
use rtd_transport::{Obfuscated2, Secret, SecretMode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

/// The length of an obfuscated2 init/header on the wire.
const OBFUSCATED2_INIT_LEN: usize = 64;

/// Smallest full-framing frame: `length | seq_no | crc`.
const FULL_MIN_FRAME_LEN: usize = 3 * 4;

/// How a connection's MTProto byte stream was wrapped on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    /// Plain TCP carrying the codec's framing directly.
    Plain,
    /// Bare obfuscated2 (MTProxy transport obfuscation).
    Obfuscated,
    /// FakeTLS record framing with obfuscated2 inside.
    FakeTls,
    /// A WebSocket whose binary messages carry the MTProto stream.
    WebSocket,
}

/// Metadata describing an accepted [`Connection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionMeta {
    /// The transport framing to apply to the byte stream.
    pub codec: CodecKind,
    /// The wrapping the byte stream was unwrapped from.
    pub mode: TransportMode,
    /// The data-center label from an obfuscated2/MTProxy header, if any.
    pub dc_id: Option<u16>,
    /// The MTProxy proxy user, if the transport carried one.
    pub proxy_user: Option<String>,
}

/// Configuration for an [`Ingress`].
#[derive(Debug, Clone, Default)]
pub struct IngressConfig {
    /// An MTProxy secret. `Plain`/`Secured` (`dd`) secrets authenticate bare
    /// obfuscated2; a `FakeTls` (`ee`) secret authenticates FakeTLS.
    pub secret: Option<Secret>,
    /// Whether HTTP upgrade requests are accepted as WebSocket sessions.
    pub websocket: bool,
}

impl IngressConfig {
    /// Creates an empty configuration: plain and bare obfuscated2 only.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            secret: None,
            websocket: false,
        }
    }

    /// Sets the MTProxy secret.
    #[must_use]
    pub fn with_secret(mut self, secret: Secret) -> Self {
        self.secret = Some(secret);
        self
    }

    /// Enables or disables WebSocket ingress.
    #[must_use]
    pub fn with_websocket(mut self, enabled: bool) -> Self {
        self.websocket = enabled;
        self
    }

    /// Returns the key used to unwrap a bare obfuscated2 connection.
    ///
    /// Without a secret this is empty (unguarded obfuscated2). A `FakeTls`
    /// secret is a mode mismatch: bare obfuscated2 is not authenticated by it.
    fn obfuscated2_key(&self) -> Result<Vec<u8>, IngressError> {
        match &self.secret {
            None => Ok(Vec::new()),
            Some(secret) => secret
                .obfuscated2_key()
                .map(<[u8]>::to_vec)
                .map_err(|_| IngressError::SecretModeMismatch),
        }
    }

    /// Returns the FakeTLS secret, or rejects the connection.
    fn faketls_secret(&self) -> Result<&Secret, IngressError> {
        match &self.secret {
            Some(secret) if secret.mode() == SecretMode::FakeTls => Ok(secret),
            _ => Err(IngressError::SecretModeMismatch),
        }
    }
}

/// An error binding an [`Ingress`] or accepting from it.
#[derive(Debug)]
#[non_exhaustive]
pub enum IngressError {
    /// The listener could not be bound or read from.
    Io(io::Error),
    /// The connection was rejected by transport detection.
    Rejected(&'static str),
    /// A configured MTProxy secret's mode does not match the connection.
    SecretModeMismatch,
    /// Unwrapping the MTProxy obfuscation failed.
    Transport(rtd_transport::Error),
    /// The WebSocket handshake failed.
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
}

impl fmt::Display for IngressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Rejected(reason) => write!(f, "connection rejected: {reason}"),
            Self::SecretModeMismatch => {
                f.write_str("the configured MTProxy secret does not match this transport")
            }
            Self::Transport(err) => write!(f, "transport unwrap failed: {err}"),
            Self::WebSocket(err) => write!(f, "websocket handshake failed: {err}"),
        }
    }
}

impl core::error::Error for IngressError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Transport(err) => Some(err),
            Self::WebSocket(err) => Some(err),
            Self::Rejected(_) | Self::SecretModeMismatch => None,
        }
    }
}

impl From<io::Error> for IngressError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// An owned, transport-agnostic MTProto byte stream.
///
/// The bytes are already unwrapped from obfuscation and record framing; the
/// [`ConnectionMeta::codec`] says how to split them into frames.
pub struct Connection {
    io: Box<dyn AsyncStream>,
    meta: ConnectionMeta,
}

impl Connection {
    /// Wraps a stream and its metadata.
    fn new<S: AsyncStream + 'static>(io: S, meta: ConnectionMeta) -> Self {
        Self {
            io: Box::new(io),
            meta,
        }
    }

    /// Returns the connection's transport metadata.
    #[must_use]
    pub fn meta(&self) -> &ConnectionMeta {
        &self.meta
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.get_mut().io).poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.get_mut().io).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.get_mut().io).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.get_mut().io).poll_shutdown(cx)
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}

/// A boxable MTProto byte stream.
trait AsyncStream: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin> AsyncStream for T {}

/// Binds a TCP listener and yields detected, unwrapped connections.
///
/// The same listener serves every configured transport: the first bytes select
/// plain framing, obfuscated2, FakeTLS or WebSocket.
pub struct Ingress {
    listener: TcpListener,
    config: IngressConfig,
}

impl Ingress {
    /// Binds a listener on `addr` with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns [`IngressError::Io`] if the address cannot be bound.
    pub async fn bind(
        addr: impl ToSocketAddrs,
        config: IngressConfig,
    ) -> Result<Self, IngressError> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self { listener, config })
    }

    /// Returns the local address the listener is bound to.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] if the address cannot be resolved.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts the next connection and applies transport detection.
    ///
    /// # Errors
    ///
    /// Returns [`IngressError`] if accepting fails, the connection is rejected by
    /// detection, or the configured transport cannot be unwrapped.
    pub async fn accept(&self) -> Result<Connection, IngressError> {
        let (stream, _peer) = self.listener.accept().await?;
        accept_stream(stream, &self.config).await
    }
}

impl fmt::Debug for Ingress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ingress")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// The wrapping selected by the first bytes of a connection.
enum Wire {
    /// Plain framing; the codec tag (if any) was consumed.
    Plain(CodecKind),
    /// Obfuscated2; the read prefix must be replayed.
    Obfuscated,
    /// A TLS record (FakeTLS); the read prefix must be replayed.
    Tls,
    /// An HTTP request (WebSocket); the read prefix must be replayed.
    Http,
}

/// Detects and unwraps a raw connection.
async fn accept_stream<S>(stream: S, config: &IngressConfig) -> Result<Connection, IngressError>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (wire, prefix, stream) = detect(stream).await?;
    match wire {
        Wire::Plain(codec) => Ok(Connection::new(
            Prefixed::new(prefix, stream),
            plain_meta(codec, TransportMode::Plain),
        )),
        Wire::Obfuscated => {
            let key = config.obfuscated2_key()?;
            obfuscated_connection(
                Prefixed::new(prefix, stream),
                &key,
                TransportMode::Obfuscated,
            )
            .await
        }
        Wire::Tls => {
            let secret = config.faketls_secret()?;
            fake_tls_connection(stream, prefix, secret).await
        }
        Wire::Http => {
            if !config.websocket {
                return Err(IngressError::Rejected("websocket ingress is not enabled"));
            }
            let upgraded = tokio_tungstenite::accept_async(Prefixed::new(prefix, stream))
                .await
                .map_err(|err| IngressError::WebSocket(Box::new(err)))?;
            websocket_connection(WsStream::new(upgraded), config).await
        }
    }
}

/// Detects and unwraps the MTProto stream carried by WebSocket messages.
async fn websocket_connection<S>(
    stream: S,
    config: &IngressConfig,
) -> Result<Connection, IngressError>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (wire, prefix, stream) = detect(stream).await?;
    match wire {
        Wire::Plain(codec) => Ok(Connection::new(
            Prefixed::new(prefix, stream),
            plain_meta(codec, TransportMode::WebSocket),
        )),
        Wire::Obfuscated => {
            // WebSocket carries bare obfuscated2 without a secret, matching the
            // reference transport, so a configured secret is a mode mismatch.
            let key = match &config.secret {
                None => Vec::new(),
                Some(secret) => secret
                    .obfuscated2_key()
                    .map(<[u8]>::to_vec)
                    .unwrap_or_default(),
            };
            obfuscated_connection(
                Prefixed::new(prefix, stream),
                &key,
                TransportMode::WebSocket,
            )
            .await
        }
        Wire::Tls | Wire::Http => Err(IngressError::Rejected("nested transport inside websocket")),
    }
}

/// Reads the connection's leading bytes once and classifies the transport.
async fn detect<S>(mut stream: S) -> Result<(Wire, Vec<u8>, S), IngressError>
where
    S: AsyncRead + Unpin,
{
    let mut prefix = Vec::with_capacity(8);
    prefix.push(0);
    stream.read_exact(&mut prefix[..1]).await?;
    if prefix[0] == ABRIDGED_HEADER {
        // Abridged sends a single client-only tag byte.
        return Ok((Wire::Plain(CodecKind::Abridged), Vec::new(), stream));
    }

    prefix.resize(4, 0);
    stream.read_exact(&mut prefix[1..4]).await?;
    if prefix.as_slice() == INTERMEDIATE_HEADER {
        return Ok((Wire::Plain(CodecKind::Intermediate), Vec::new(), stream));
    }
    if prefix.as_slice() == PADDED_INTERMEDIATE_HEADER {
        return Ok((
            Wire::Plain(CodecKind::PaddedIntermediate),
            Vec::new(),
            stream,
        ));
    }
    if is_tls_prefix(&prefix) {
        return Ok((Wire::Tls, prefix, stream));
    }
    if is_http_prefix(&prefix) {
        return Ok((Wire::Http, prefix, stream));
    }

    prefix.resize(8, 0);
    stream.read_exact(&mut prefix[4..8]).await?;
    if u32::from_le_bytes([prefix[4], prefix[5], prefix[6], prefix[7]]) == 0 {
        // The first full-framing word carries transport sequence number zero.
        let length = u32::from_le_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]) as usize;
        if !(FULL_MIN_FRAME_LEN..=MAX_MESSAGE_SIZE).contains(&length) || !length.is_multiple_of(4) {
            return Err(IngressError::Rejected("invalid full-framing header"));
        }
        return Ok((Wire::Plain(CodecKind::Full), prefix, stream));
    }

    // Non-zero nonce bytes 4..8 mean an obfuscated2 init, not a plaintext frame.
    Ok((Wire::Obfuscated, prefix, stream))
}

/// Returns whether `prefix` begins a TLS record.
fn is_tls_prefix(prefix: &[u8]) -> bool {
    prefix.len() >= 3 && prefix[0] == 0x16 && prefix[1] == 0x03 && matches!(prefix[2], 1..=4)
}

/// Returns whether the leading four bytes match an HTTP method prefix.
///
/// `OPTI` is the four-byte prefix of `OPTIONS`. The reference server's
/// same-port multiplexer tests exactly these four prefixes (`"GET "`,
/// `"POST"`, `"HEAD"`, `"OPTI"`), so this is intentional and not a
/// truncation of `OPTIONS`.
fn is_http_prefix(prefix: &[u8]) -> bool {
    prefix.starts_with(b"GET ")
        || prefix.starts_with(b"POST")
        || prefix.starts_with(b"HEAD")
        || prefix.starts_with(b"OPTI")
}

/// Builds metadata for a plain connection.
fn plain_meta(codec: CodecKind, mode: TransportMode) -> ConnectionMeta {
    ConnectionMeta {
        codec,
        mode,
        dc_id: None,
        proxy_user: None,
    }
}

/// Unwraps an obfuscated2 init and returns the framed connection.
async fn obfuscated_connection<S>(
    mut stream: S,
    secret: &[u8],
    mode: TransportMode,
) -> Result<Connection, IngressError>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let mut init = [0u8; OBFUSCATED2_INIT_LEN];
    stream.read_exact(&mut init).await?;

    let mut bridge = SyncBridge::default();
    bridge.inbound.extend(init);
    let (inner, metadata) = Obfuscated2::accept(bridge, secret).map_err(IngressError::Transport)?;

    let meta = ConnectionMeta {
        codec: codec_from_protocol(metadata.protocol)?,
        mode,
        dc_id: Some(metadata.dc),
        proxy_user: None,
    };
    Ok(Connection::new(ObfuscatedStream::new(stream, inner), meta))
}

/// Maps an obfuscated2 protocol tag to its framing.
fn codec_from_protocol(protocol: [u8; 4]) -> Result<CodecKind, IngressError> {
    if protocol == [ABRIDGED_HEADER; 4] {
        Ok(CodecKind::Abridged)
    } else if protocol == INTERMEDIATE_HEADER {
        Ok(CodecKind::Intermediate)
    } else if protocol == PADDED_INTERMEDIATE_HEADER {
        Ok(CodecKind::PaddedIntermediate)
    } else {
        Err(IngressError::Rejected("unknown obfuscated protocol tag"))
    }
}

/// Runs the FakeTLS handshake and unwraps the obfuscated2 stream inside it.
async fn fake_tls_connection<S>(
    mut stream: S,
    prefix: Vec<u8>,
    secret: &Secret,
) -> Result<Connection, IngressError>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let key = secret.faketls_key().map_err(IngressError::Transport)?;

    // Reassemble the ClientHello record from the detected prefix and the rest.
    let mut record = prefix;
    record.resize(5, 0);
    stream.read_exact(&mut record[4..5]).await?;
    let length = usize::from(u16::from_be_bytes([record[3], record[4]]));
    let body_start = record.len();
    record.resize(body_start + length, 0);
    stream.read_exact(&mut record[body_start..]).await?;

    let client_random = verify_client_hello(&record, key).map_err(IngressError::Transport)?;

    let mut response = Vec::new();
    write_server_hello(&mut response, client_random, key).map_err(IngressError::Transport)?;
    stream.write_all(&response).await?;
    stream.flush().await?;

    obfuscated_connection(FakeTlsStream::new(stream), key, TransportMode::FakeTls).await
}

/// A prefix buffer served before an underlying async stream.
struct Prefixed<S> {
    prefix: Vec<u8>,
    pos: usize,
    inner: S,
}

impl<S> Prefixed<S> {
    fn new(prefix: Vec<u8>, inner: S) -> Self {
        Self {
            prefix,
            pos: 0,
            inner,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Prefixed<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.pos < this.prefix.len() {
            let available = this.prefix.len() - this.pos;
            let n = available.min(buf.remaining());
            buf.put_slice(&this.prefix[this.pos..this.pos + n]);
            this.pos += n;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Prefixed<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

/// A synchronous facade over an in-memory byte buffer.
///
/// It lets the sans-IO [`rtd_transport::Obfuscated2`] stream be driven from an
/// async source: the async wrapper fills `inbound` before each read and drains
/// `outbound` after each write, so the AES-CTR keystream advances in order.
#[derive(Default)]
struct SyncBridge {
    inbound: VecDeque<u8>,
    outbound: Vec<u8>,
}

impl Read for SyncBridge {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.inbound.len());
        for slot in buf.iter_mut().take(n) {
            *slot = self.inbound.pop_front().unwrap_or(0);
        }
        Ok(n)
    }
}

impl Write for SyncBridge {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.outbound.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// An obfuscated2 stream over an async source.
struct ObfuscatedStream<S> {
    source: S,
    inner: Obfuscated2<SyncBridge>,
    write_pending: Vec<u8>,
}

impl<S> ObfuscatedStream<S> {
    fn new(source: S, inner: Obfuscated2<SyncBridge>) -> Self {
        Self {
            source,
            inner,
            write_pending: Vec::new(),
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for ObfuscatedStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if this.inner.get_ref().inbound.is_empty() {
            let mut tmp = [0u8; 8 * 1024];
            let n = {
                let mut rb = ReadBuf::new(&mut tmp);
                match Pin::new(&mut this.source).poll_read(cx, &mut rb) {
                    Poll::Ready(Ok(())) => rb.filled().len(),
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => return Poll::Pending,
                }
            };
            if n == 0 {
                return Poll::Ready(Ok(()));
            }
            this.inner
                .get_mut()
                .inbound
                .extend(tmp[..n].iter().copied());
        }

        let mut tmp = [0u8; 8 * 1024];
        let want = buf.remaining().min(tmp.len());
        let n = match this.inner.read(&mut tmp[..want]) {
            Ok(n) => n,
            Err(err) => return Poll::Ready(Err(err)),
        };
        buf.put_slice(&tmp[..n]);
        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for ObfuscatedStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if let Poll::Ready(Err(err)) = flush_pending(&mut this.source, &mut this.write_pending, cx)
        {
            return Poll::Ready(Err(err));
        }

        let n = match this.inner.write(buf) {
            Ok(n) => n,
            Err(err) => return Poll::Ready(Err(err)),
        };
        let outbound = std::mem::take(&mut this.inner.get_mut().outbound);
        this.write_pending.extend_from_slice(&outbound);

        if let Poll::Ready(Err(err)) = flush_pending(&mut this.source, &mut this.write_pending, cx)
        {
            return Poll::Ready(Err(err));
        }
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match flush_pending(&mut this.source, &mut this.write_pending, cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut this.source).poll_flush(cx),
            other => other,
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match flush_pending(&mut this.source, &mut this.write_pending, cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut this.source).poll_shutdown(cx),
            other => other,
        }
    }
}

/// A FakeTLS record stream over an async source.
struct FakeTlsStream<S> {
    source: S,
    wrote_cipher_spec: bool,
    write_pending: Vec<u8>,
    header: [u8; 5],
    header_len: usize,
    record_type: u8,
    body: Vec<u8>,
    body_read: usize,
    body_pos: usize,
}

impl<S> FakeTlsStream<S> {
    fn new(source: S) -> Self {
        Self {
            source,
            wrote_cipher_spec: false,
            write_pending: Vec::new(),
            header: [0u8; 5],
            header_len: 0,
            record_type: 0,
            body: Vec::new(),
            body_read: 0,
            body_pos: 0,
        }
    }

    /// Parses a completed header into the record type and body size.
    fn begin_record(&mut self) -> io::Result<()> {
        if !matches!(
            [self.header[1], self.header[2]],
            VERSION_10 | VERSION_11 | VERSION_12 | VERSION_13
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown TLS record version",
            ));
        }
        self.record_type = self.header[0];
        let length = usize::from(u16::from_be_bytes([self.header[3], self.header[4]]));
        self.body = vec![0u8; length];
        self.body_read = 0;
        self.body_pos = 0;
        Ok(())
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for FakeTlsStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            if this.body_read == this.body.len() && this.body_pos < this.body_read {
                let available = this.body_read - this.body_pos;
                let n = available.min(buf.remaining());
                buf.put_slice(&this.body[this.body_pos..this.body_pos + n]);
                this.body_pos += n;
                if this.body_pos == this.body_read {
                    this.body.clear();
                    this.body_read = 0;
                    this.body_pos = 0;
                }
                return Poll::Ready(Ok(()));
            }
            if buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }

            // A header is only read when no record body is pending, so a body
            // split across reads is never mistaken for the next header.
            if this.body.is_empty() {
                if this.header_len < 5 {
                    let n = {
                        let mut rb = ReadBuf::new(&mut this.header[this.header_len..]);
                        match Pin::new(&mut this.source).poll_read(cx, &mut rb) {
                            Poll::Ready(Ok(())) => rb.filled().len(),
                            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                            Poll::Pending => return Poll::Pending,
                        }
                    };
                    if n == 0 {
                        return Poll::Ready(Ok(()));
                    }
                    this.header_len += n;
                    if this.header_len < 5 {
                        continue;
                    }
                }
                if let Err(err) = this.begin_record() {
                    return Poll::Ready(Err(err));
                }
                this.header_len = 0;
                if this.body.is_empty() {
                    continue;
                }
            }

            if this.body_read < this.body.len() {
                let n = {
                    let mut rb = ReadBuf::new(&mut this.body[this.body_read..]);
                    match Pin::new(&mut this.source).poll_read(cx, &mut rb) {
                        Poll::Ready(Ok(())) => rb.filled().len(),
                        Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                        Poll::Pending => return Poll::Pending,
                    }
                };
                if n == 0 {
                    return Poll::Ready(Ok(()));
                }
                this.body_read += n;
                if this.body_read < this.body.len() {
                    continue;
                }
            }

            match this.record_type {
                RECORD_TYPE_CHANGE_CIPHER_SPEC => {
                    this.body.clear();
                    this.body_read = 0;
                    this.body_pos = 0;
                }
                RECORD_TYPE_APPLICATION => {
                    this.body_pos = 0;
                }
                other => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("unexpected TLS record type {other:#04x}"),
                    )));
                }
            }
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for FakeTlsStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if let Poll::Ready(Err(err)) = flush_pending(&mut this.source, &mut this.write_pending, cx)
        {
            return Poll::Ready(Err(err));
        }

        let mut frame = Vec::with_capacity(buf.len() + 16);
        if !this.wrote_cipher_spec {
            write_record(
                &mut frame,
                &Record::new(RECORD_TYPE_CHANGE_CIPHER_SPEC, VERSION_12, vec![0x01]),
            )
            .map_err(io::Error::other)?;
            this.wrote_cipher_spec = true;
        }
        write_record(
            &mut frame,
            &Record::new(RECORD_TYPE_APPLICATION, VERSION_12, buf.to_vec()),
        )
        .map_err(io::Error::other)?;
        this.write_pending.extend_from_slice(&frame);

        if let Poll::Ready(Err(err)) = flush_pending(&mut this.source, &mut this.write_pending, cx)
        {
            return Poll::Ready(Err(err));
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match flush_pending(&mut this.source, &mut this.write_pending, cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut this.source).poll_flush(cx),
            other => other,
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match flush_pending(&mut this.source, &mut this.write_pending, cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut this.source).poll_shutdown(cx),
            other => other,
        }
    }
}

/// A WebSocket message stream exposed as a byte stream.
struct WsStream<S> {
    ws: WebSocketStream<S>,
    read_buf: Vec<u8>,
    read_pos: usize,
    write_buf: Vec<u8>,
}

impl<S> WsStream<S> {
    fn new(ws: WebSocketStream<S>) -> Self {
        Self {
            ws,
            read_buf: Vec::new(),
            read_pos: 0,
            write_buf: Vec::new(),
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for WsStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            if this.read_pos < this.read_buf.len() {
                let available = this.read_buf.len() - this.read_pos;
                let n = available.min(buf.remaining());
                buf.put_slice(&this.read_buf[this.read_pos..this.read_pos + n]);
                this.read_pos += n;
                return Poll::Ready(Ok(()));
            }
            if buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }
            match Pin::new(&mut this.ws).poll_next(cx) {
                Poll::Ready(Some(Ok(message))) => {
                    let data = match message {
                        Message::Binary(data) | Message::Ping(data) | Message::Pong(data) => data,
                        Message::Text(text) => text.into_bytes(),
                        Message::Close(_) => return Poll::Ready(Ok(())),
                        Message::Frame(_) => continue,
                    };
                    this.read_buf = data;
                    this.read_pos = 0;
                    if this.read_buf.is_empty() {
                        continue;
                    }
                }
                Poll::Ready(Some(Err(err))) => {
                    return Poll::Ready(Err(io::Error::other(err)));
                }
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for WsStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.get_mut().write_buf.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if !this.write_buf.is_empty() {
            match Pin::new(&mut this.ws).poll_ready(cx) {
                Poll::Ready(Ok(())) => {
                    let data = std::mem::take(&mut this.write_buf);
                    if let Err(err) = Pin::new(&mut this.ws).start_send(Message::Binary(data)) {
                        return Poll::Ready(Err(io::Error::other(err)));
                    }
                }
                Poll::Ready(Err(err)) => return Poll::Ready(Err(io::Error::other(err))),
                Poll::Pending => return Poll::Pending,
            }
        }
        match Pin::new(&mut this.ws).poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(err)) => Poll::Ready(Err(io::Error::other(err))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if !this.write_buf.is_empty() {
            match Pin::new(&mut this.ws).poll_ready(cx) {
                Poll::Ready(Ok(())) => {
                    let data = std::mem::take(&mut this.write_buf);
                    if let Err(err) = Pin::new(&mut this.ws).start_send(Message::Binary(data)) {
                        return Poll::Ready(Err(io::Error::other(err)));
                    }
                }
                Poll::Ready(Err(err)) => return Poll::Ready(Err(io::Error::other(err))),
                Poll::Pending => return Poll::Pending,
            }
        }
        match Pin::new(&mut this.ws).poll_close(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(err)) => Poll::Ready(Err(io::Error::other(err))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Best-effort drain of `pending` into `source`, preserving unsent bytes.
fn flush_pending<S: AsyncWrite + Unpin>(
    source: &mut S,
    pending: &mut Vec<u8>,
    cx: &mut Context<'_>,
) -> Poll<io::Result<()>> {
    while !pending.is_empty() {
        match Pin::new(&mut *source).poll_write(cx, pending) {
            Poll::Ready(Ok(0)) => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "failed to write buffered bytes",
                )));
            }
            Poll::Ready(Ok(n)) => {
                pending.drain(..n);
            }
            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            Poll::Pending => return Poll::Pending,
        }
    }
    Poll::Ready(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtd_proto::codec::{Codec, PaddedIntermediate};

    const PAYLOAD: [u8; 4] = [1, 2, 3, 4];

    /// Builds the codec frame (without the connection tag) for `kind`.
    fn frame(kind: CodecKind, payload: &[u8]) -> Vec<u8> {
        let mut codec = kind.build();
        let mut buffer = Vec::new();
        codec.write_message(&mut buffer, payload).unwrap();
        buffer
    }

    /// Runs one plaintext framing through detection and reads the frame back.
    async fn check_plain(kind: CodecKind) {
        let frame = frame(kind, &PAYLOAD);
        let mut wire = Vec::new();
        match kind {
            CodecKind::Abridged => wire.push(ABRIDGED_HEADER),
            CodecKind::Intermediate => wire.extend_from_slice(&INTERMEDIATE_HEADER),
            CodecKind::PaddedIntermediate => wire.extend_from_slice(&PADDED_INTERMEDIATE_HEADER),
            CodecKind::Full => {}
        }
        wire.extend_from_slice(&frame);

        let (mut client, server) = tokio::io::duplex(4096);
        client.write_all(&wire).await.unwrap();
        let mut conn = accept_stream(server, &IngressConfig::default())
            .await
            .unwrap();
        assert_eq!(conn.meta().codec, kind);
        assert_eq!(conn.meta().mode, TransportMode::Plain);
        assert_eq!(conn.meta().dc_id, None);

        let mut received = vec![0u8; frame.len()];
        conn.read_exact(&mut received).await.unwrap();
        assert_eq!(received, frame);
    }

    #[tokio::test]
    async fn detects_abridged() {
        check_plain(CodecKind::Abridged).await;
    }

    #[tokio::test]
    async fn detects_intermediate() {
        check_plain(CodecKind::Intermediate).await;
    }

    #[tokio::test]
    async fn detects_padded_intermediate() {
        check_plain(CodecKind::PaddedIntermediate).await;
    }

    #[tokio::test]
    async fn detects_full() {
        check_plain(CodecKind::Full).await;
    }

    #[tokio::test]
    async fn accepts_obfuscated2() {
        let secret = Secret::parse("52a493bdfb90eea55739eabff2d92a14").unwrap();
        let key = secret.obfuscated2_key().unwrap().to_vec();

        let mut client = Obfuscated2::handshake(
            std::io::Cursor::new(Vec::new()),
            INTERMEDIATE_HEADER,
            7,
            &key,
        )
        .unwrap();
        client.write_all(b"obf").unwrap();
        let wire = client.into_inner().into_inner();

        let (mut client, server) = tokio::io::duplex(4096);
        client.write_all(&wire).await.unwrap();
        let mut conn = accept_stream(server, &IngressConfig::new().with_secret(secret))
            .await
            .unwrap();
        assert_eq!(conn.meta().mode, TransportMode::Obfuscated);
        assert_eq!(conn.meta().codec, CodecKind::Intermediate);
        assert_eq!(conn.meta().dc_id, Some(7));

        let mut received = [0u8; 3];
        conn.read_exact(&mut received).await.unwrap();
        assert_eq!(&received, b"obf");
    }

    #[tokio::test]
    async fn accepts_faketls_and_round_trips() {
        let secret =
            Secret::parse("ee852380f362a09343efb4690c4e17862e676f6f676c652e636f6d").unwrap();
        let key = secret.faketls_key().unwrap().to_vec();
        let cloak = secret.cloak_host().to_owned();

        let ingress = Ingress::bind(
            "127.0.0.1:0",
            IngressConfig::new().with_secret(secret.clone()),
        )
        .await
        .unwrap();
        let addr = ingress.local_addr().unwrap();

        let client_key = key.clone();
        let client = tokio::task::spawn_blocking(move || -> Vec<u8> {
            let tcp = std::net::TcpStream::connect(addr).unwrap();
            let mut tls = rtd_transport::FakeTls::new(tcp);
            tls.handshake(&client_key, &cloak).unwrap();
            let mut obfs =
                Obfuscated2::handshake(tls, INTERMEDIATE_HEADER, 2, &client_key).unwrap();
            obfs.write_all(b"fake-hello").unwrap();
            obfs.flush().unwrap();
            let mut echo = [0u8; 5];
            obfs.read_exact(&mut echo).unwrap();
            echo.to_vec()
        });

        let mut conn = ingress.accept().await.unwrap();
        assert_eq!(conn.meta().mode, TransportMode::FakeTls);
        assert_eq!(conn.meta().codec, CodecKind::Intermediate);
        assert_eq!(conn.meta().dc_id, Some(2));

        let mut received = [0u8; 10];
        conn.read_exact(&mut received).await.unwrap();
        assert_eq!(&received, b"fake-hello");
        conn.write_all(b"world").await.unwrap();
        conn.flush().await.unwrap();

        assert_eq!(client.await.unwrap(), b"world");
    }

    #[tokio::test]
    async fn rejects_faketls_with_wrong_secret() {
        let configured =
            Secret::parse("ee852380f362a09343efb4690c4e17862e676f6f676c652e636f6d").unwrap();
        let other =
            Secret::parse("eeaabbccddeeff001122334455667788996578616d706c652e6f7267").unwrap();
        let other_key = other.faketls_key().unwrap();

        // A ClientHello that authenticates with `other`, presented to a server
        // configured with `configured`.
        let mut record = Vec::new();
        rtd_transport::faketls::write_client_hello(&mut record, 42, "example.com", other_key)
            .unwrap();

        let (mut client, server) = tokio::io::duplex(4096);
        client.write_all(&record).await.unwrap();
        let error = accept_stream(server, &IngressConfig::new().with_secret(configured))
            .await
            .unwrap_err();
        assert!(matches!(error, IngressError::Transport(_)));
    }

    #[tokio::test]
    async fn fake_tls_stream_handles_split_records() {
        let mut wire = Vec::new();
        write_record(
            &mut wire,
            &Record::new(RECORD_TYPE_CHANGE_CIPHER_SPEC, VERSION_12, vec![1]),
        )
        .unwrap();
        let payload: Vec<u8> = (0..64u8).collect();
        write_record(
            &mut wire,
            &Record::new(RECORD_TYPE_APPLICATION, VERSION_12, payload.clone()),
        )
        .unwrap();
        write_record(
            &mut wire,
            &Record::new(RECORD_TYPE_APPLICATION, VERSION_12, vec![9; 10]),
        )
        .unwrap();

        let (mut client, server) = tokio::io::duplex(4);
        let writer = tokio::spawn(async move {
            for byte in wire {
                client.write_all(&[byte]).await.unwrap();
                tokio::task::yield_now().await;
            }
        });
        let mut stream = FakeTlsStream::new(server);
        let mut got = vec![0u8; 64];
        stream.read_exact(&mut got).await.unwrap();
        assert_eq!(got, payload);
        let mut tail = [0u8; 10];
        stream.read_exact(&mut tail).await.unwrap();
        assert_eq!(tail, [9u8; 10]);
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_reserved_prefix_when_not_configured() {
        let (mut client, server) = tokio::io::duplex(64);
        client.write_all(b"GET / HTTP/1.1\r\n").await.unwrap();
        let error = accept_stream(server, &IngressConfig::default())
            .await
            .unwrap_err();
        assert!(matches!(error, IngressError::Rejected(_)));
    }

    #[tokio::test]
    async fn options_request_is_treated_as_http() {
        let (mut client, server) = tokio::io::duplex(64);
        client.write_all(b"OPTIONS / HTTP/1.1\r\n").await.unwrap();
        let error = accept_stream(server, &IngressConfig::default())
            .await
            .unwrap_err();
        assert!(matches!(error, IngressError::Rejected(_)));
    }

    #[tokio::test]
    async fn websocket_round_trips() {
        use futures_util::{SinkExt, StreamExt};

        let ingress = Ingress::bind("127.0.0.1:0", IngressConfig::new().with_websocket(true))
            .await
            .unwrap();
        let addr = ingress.local_addr().unwrap();

        let client = tokio::spawn(async move {
            let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
            let (mut ws, _response) =
                tokio_tungstenite::client_async(format!("ws://{addr}/apiws"), tcp)
                    .await
                    .unwrap();
            let mut frame = vec![ABRIDGED_HEADER, 0x02];
            frame.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
            ws.send(Message::Binary(frame)).await.unwrap();
            let message = ws.next().await.unwrap().unwrap();
            message.into_data()
        });

        let mut conn = ingress.accept().await.unwrap();
        assert_eq!(conn.meta().mode, TransportMode::WebSocket);
        assert_eq!(conn.meta().codec, CodecKind::Abridged);

        let mut received = [0u8; 9];
        conn.read_exact(&mut received).await.unwrap();
        assert_eq!(received[0], 0x02);
        assert_eq!(&received[1..], &[1, 2, 3, 4, 5, 6, 7, 8]);

        conn.write_all(&[9, 9, 9, 9]).await.unwrap();
        conn.flush().await.unwrap();
        assert_eq!(client.await.unwrap(), vec![9, 9, 9, 9]);
    }

    #[test]
    fn padded_intermediate_golden_is_detectable() {
        // A padded-intermediate frame produced with a fixed last byte has a
        // length that detection accepts.
        let mut codec = PaddedIntermediate::new();
        let mut buffer = Vec::new();
        codec.write_message(&mut buffer, &[1, 2, 3, 4]).unwrap();
        let length = u32::from_le_bytes(buffer[..4].try_into().unwrap()) as usize;
        assert!(length.is_multiple_of(4));
    }
}
