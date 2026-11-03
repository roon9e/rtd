//! Framed byte transports for the client.

use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, Sender, channel};

use rtd_proto::{Abridged, Codec, CodecKind, Full, Intermediate, PaddedIntermediate};
use rtd_transport::{DefaultDialer, Dialer as _, Obfuscated2, Secret};

use crate::error::{Error, Result};

/// A framed, bidirectional transport: whole MTProto packets in and out.
///
/// Implementations hide whether the bytes travel over a socket, an obfuscated
/// connection or an in-memory duplex.
pub trait Transport {
    /// Writes one already-framed packet.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Closed`] or [`Error::Transport`] when the peer is gone
    /// or the underlying I/O fails.
    fn send_frame(&mut self, frame: &[u8]) -> Result<()>;

    /// Reads one framed packet, blocking until one is available.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Closed`] or [`Error::Transport`] on a closed or failing
    /// connection.
    fn recv_frame(&mut self) -> Result<Vec<u8>>;
}

/// Reads framed packets independently of any writer.
pub trait ReadTransport {
    /// Reads one framed packet, blocking until one is available.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Closed`] or [`Error::Transport`] on a closed or failing
    /// connection.
    fn recv_frame(&mut self) -> Result<Vec<u8>>;
}

/// Writes framed packets independently of any reader.
pub trait WriteTransport {
    /// Writes one already-framed packet.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Closed`] or [`Error::Transport`] when the peer is gone
    /// or the underlying I/O fails.
    fn send_frame(&mut self, frame: &[u8]) -> Result<()>;
}

impl<T: Transport> ReadTransport for T {
    fn recv_frame(&mut self) -> Result<Vec<u8>> {
        Transport::recv_frame(self)
    }
}

impl<T: Transport> WriteTransport for T {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        Transport::send_frame(self, frame)
    }
}

/// An in-memory duplex: the two ends of [`DuplexStream::pair`] exchange frames
/// without a socket, so the client can be driven against an in-process peer.
pub struct DuplexStream {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
}

impl DuplexStream {
    /// Creates two connected ends.
    #[must_use]
    pub fn pair() -> (Self, Self) {
        let (a_tx, b_rx) = channel();
        let (b_tx, a_rx) = channel();
        (Self { tx: a_tx, rx: a_rx }, Self { tx: b_tx, rx: b_rx })
    }

    /// Splits the duplex into independent read and write halves.
    #[must_use]
    pub fn split(self) -> (DuplexReader, DuplexWriter) {
        (DuplexReader { rx: self.rx }, DuplexWriter { tx: self.tx })
    }
}

/// The read half of a split [`DuplexStream`].
pub struct DuplexReader {
    rx: Receiver<Vec<u8>>,
}

impl ReadTransport for DuplexReader {
    fn recv_frame(&mut self) -> Result<Vec<u8>> {
        self.rx.recv().map_err(|_| Error::Closed)
    }
}

/// The write half of a split [`DuplexStream`].
pub struct DuplexWriter {
    tx: Sender<Vec<u8>>,
}

impl WriteTransport for DuplexWriter {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.tx.send(frame.to_vec()).map_err(|_| Error::Closed)
    }
}

impl Transport for DuplexStream {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.tx.send(frame.to_vec()).map_err(|_| Error::Closed)
    }

    fn recv_frame(&mut self) -> Result<Vec<u8>> {
        self.rx.recv().map_err(|_| Error::Closed)
    }
}

/// A boxed byte stream that a [`Framed`] transport can read and write.
///
/// The `Send` bound lets a transport move to the asynchronous actor thread.
pub trait ReadWrite: Read + Write + Send {}

impl<T: Read + Write + Send> ReadWrite for T {}

/// Builds a codec boxed with an explicit `Send` bound (the trait object from
/// `CodecKind::build` is not `Send`).
fn build_codec(kind: CodecKind) -> Box<dyn Codec + Send> {
    match kind {
        CodecKind::Abridged => Box::new(Abridged),
        CodecKind::Intermediate => Box::new(Intermediate),
        CodecKind::PaddedIntermediate => Box::new(PaddedIntermediate::new()),
        CodecKind::Full => Box::new(Full::new()),
    }
}

/// A framed transport over a boxed byte stream.
pub struct Framed<IO> {
    io: IO,
    codec: Box<dyn Codec + Send>,
}

impl<IO: Read + Write> Framed<IO> {
    /// Wraps `io` with the framing `codec`.
    ///
    /// When `write_header` is set the transport header is written once (plain
    /// TCP); an obfuscated connection carries its framing tag itself and passes
    /// `false`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transport`] when the header cannot be written.
    pub fn new(mut io: IO, codec: CodecKind, write_header: bool) -> Result<Self> {
        let mut codec = build_codec(codec);
        if write_header {
            codec
                .write_header(&mut io)
                .map_err(|error| Error::Transport(error.to_string()))?;
        }
        Ok(Self { io, codec })
    }
}

impl<IO: Read + Write> Transport for Framed<IO> {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.codec
            .write_message(&mut self.io, frame)
            .map_err(|error| Error::Transport(error.to_string()))
    }

    fn recv_frame(&mut self) -> Result<Vec<u8>> {
        self.codec
            .read_message(&mut self.io)
            .map_err(|error| Error::Transport(error.to_string()))
    }
}

/// A TCP transport: a socket with a framing codec and optional obfuscation.
pub type TcpTransport = Framed<Box<dyn ReadWrite>>;

/// Dials `address` and returns a framed TCP transport.
///
/// With `obfuscation` unset the codec header is written in the clear. With it
/// set the connection is obfuscated (or FakeTLS, per the secret) and the framing
/// tag is carried by the obfuscation; the secret itself may be empty for an
/// unauthenticated MTProxy-style connection. `dc` selects the obfuscation
/// datacenter salt.
///
/// # Errors
///
/// Returns [`Error::Io`] when the socket cannot be created or configured,
/// [`Error::Transport`] when obfuscation fails, and an error when full framing
/// is combined with obfuscation (it has no tag).
pub fn connect_tcp(
    address: &str,
    dc: u16,
    codec: CodecKind,
    obfuscation: Option<&[u8]>,
) -> Result<TcpTransport> {
    let socket = std::net::TcpStream::connect(address)?;
    socket.set_nodelay(true)?;
    let Some(secret) = obfuscation else {
        return Framed::new(Box::new(socket) as Box<dyn ReadWrite>, codec, true);
    };
    let tag = codec
        .obfuscated_tag()
        .ok_or_else(|| Error::Transport("full framing has no obfuscation tag".to_owned()))?;
    let stream = Obfuscated2::handshake(socket, tag, dc, secret)
        .map_err(|error| Error::Transport(error.to_string()))?;
    Framed::new(Box::new(stream) as Box<dyn ReadWrite>, codec, false)
}

/// Dials `address` and applies the MTProxy layer selected by `secret`.
///
/// Unlike [`connect_tcp`], this selects FakeTLS for an `ee` secret and derives
/// the keys from the parsed secret; plain TCP is [`connect_tcp`] with no
/// `obfuscation`.
///
/// # Errors
///
/// Returns [`Error::Io`] when the socket cannot be created, [`Error::Transport`]
/// when the handshake fails, and an error for full framing (it has no tag).
pub fn dial_tcp(address: &str, dc: u16, codec: CodecKind, secret: &Secret) -> Result<TcpTransport> {
    let tag = codec
        .obfuscated_tag()
        .ok_or_else(|| Error::Transport("full framing has no obfuscation tag".to_owned()))?;
    let stream = DefaultDialer
        .dial(address, dc, tag, secret)
        .map_err(|error| Error::Transport(error.to_string()))?;
    Framed::new(Box::new(stream) as Box<dyn ReadWrite>, codec, false)
}

/// A background reader draining a [`ReadTransport`] on its own thread.
///
/// This lets a client keep reading (and receiving unsolicited updates) while a
/// caller writes on the other half of a split transport.
pub struct Reader {
    rx: Receiver<Result<Vec<u8>>>,
    handle: std::thread::JoinHandle<()>,
}

impl Reader {
    /// Spawns a reader over `transport`.
    pub fn spawn<T: ReadTransport + Send + 'static>(mut transport: T) -> Self {
        let (tx, rx) = channel();
        let handle = std::thread::spawn(move || {
            loop {
                match transport.recv_frame() {
                    Ok(frame) => {
                        if tx.send(Ok(frame)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = tx.send(Err(error));
                        break;
                    }
                }
            }
        });
        Self { rx, handle }
    }

    /// Returns the next frame, or an error when the reader stopped.
    ///
    /// # Errors
    ///
    /// Returns the reader's terminal error, or [`Error::Closed`] once it stops.
    pub fn recv(&self) -> Result<Vec<u8>> {
        self.rx.recv().map_err(|_| Error::Closed)?
    }

    /// Waits for the reader thread to finish.
    ///
    /// # Errors
    ///
    /// Reserved for future use; returns `Ok(())`.
    pub fn join(self) -> Result<()> {
        let _ = self.handle.join();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DuplexStream, Transport};

    #[test]
    fn duplex_moves_frames_both_ways() {
        let (mut a, mut b) = DuplexStream::pair();
        a.send_frame(b"one").unwrap();
        b.send_frame(b"two").unwrap();
        assert_eq!(a.recv_frame().unwrap(), b"two");
        assert_eq!(b.recv_frame().unwrap(), b"one");
    }

    #[test]
    fn reader_drains_a_split_transport() {
        use super::{Reader, WriteTransport};

        let (a, b) = super::DuplexStream::pair();
        let (_, mut a_write) = a.split();
        let (b_read, _) = b.split();
        let reader = Reader::spawn(b_read);
        a_write.send_frame(b"ping").unwrap();
        assert_eq!(reader.recv().unwrap(), b"ping");
    }

    #[test]
    fn split_duplex_moves_frames_both_ways() {
        use super::{ReadTransport, WriteTransport};

        let (a, b) = super::DuplexStream::pair();
        let (mut a_read, mut a_write) = a.split();
        let (mut b_read, mut b_write) = b.split();
        a_write.send_frame(b"hi").unwrap();
        assert_eq!(b_read.recv_frame().unwrap(), b"hi");
        b_write.send_frame(b"yo").unwrap();
        assert_eq!(a_read.recv_frame().unwrap(), b"yo");
    }

    #[test]
    fn a_dropped_peer_reads_as_closed() {
        let (a, b) = DuplexStream::pair();
        drop(b);
        let mut a = a;
        assert!(matches!(a.send_frame(b"x"), Err(super::Error::Closed)));
    }
}
