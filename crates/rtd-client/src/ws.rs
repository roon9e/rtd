//! A minimal RFC 6455 WebSocket client transport.
//!
//! MTProto-over-WebSocket carries the same byte stream as TCP, one binary
//! WebSocket frame per `write`. [`WebSocketIo`] performs the HTTP upgrade and
//! the frame codec, and implements [`Read`]/[`Write`] so it composes with
//! [`Framed`](crate::transport::Framed) and the intermediate codec.

use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;

use rtd_proto::CodecKind;

use crate::error::{Error, Result};
use crate::transport::{Framed, ReadWrite, TcpTransport};

/// Largest WebSocket frame accepted from the server.
const MAX_FRAME: usize = 16 * 1024 * 1024;

/// Largest HTTP handshake response accepted.
const MAX_HANDSHAKE_HEAD: usize = 16 * 1024;

/// Byte value for a binary frame.
const OPCODE_BINARY: u8 = 0x2;
/// Byte value for a text frame (rejected: MTProto is binary).
const OPCODE_TEXT: u8 = 0x1;
/// Byte value for a continuation frame.
const OPCODE_CONTINUATION: u8 = 0x0;
/// Byte value for a close frame.
const OPCODE_CLOSE: u8 = 0x8;
/// Byte value for a ping frame.
const OPCODE_PING: u8 = 0x9;
/// Byte value for a pong frame.
const OPCODE_PONG: u8 = 0xA;

/// A WebSocket connection exposed as a byte stream.
pub struct WebSocketIo<S> {
    io: S,
    buffer: Vec<u8>,
    pos: usize,
}

impl WebSocketIo<TcpStream> {
    /// Connects to `address`, performing the HTTP upgrade for `path` with `host`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on a socket failure or [`Error::Transport`] when
    /// the server does not accept the upgrade.
    pub fn connect(address: &str, host: &str, path: &str) -> Result<Self> {
        let mut io = TcpStream::connect(address)?;
        io.set_nodelay(true)?;

        let key = base64(&rtd_crypto::random_bytes(16));
        let request = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n"
        );
        io.write_all(request.as_bytes())?;

        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if head.len() >= MAX_HANDSHAKE_HEAD {
                return Err(Error::Transport("websocket handshake too large".to_owned()));
            }
            io.read_exact(&mut byte)?;
            head.push(byte[0]);
            if head.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let head_text = String::from_utf8_lossy(&head);
        let status_ok =
            head_text.starts_with("HTTP/1.1 101") || head_text.starts_with("HTTP/1.0 101");
        if !status_ok {
            let line = head_text.lines().next().unwrap_or("").to_owned();
            return Err(Error::Transport(format!(
                "websocket upgrade failed: {line}"
            )));
        }

        Ok(Self {
            io,
            buffer: Vec::new(),
            pos: 0,
        })
    }
}

impl<S: Write> WebSocketIo<S> {
    /// Writes one masked binary frame.
    fn write_frame(&mut self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        let mut header = Vec::with_capacity(14);
        header.push(0x80 | opcode);
        let len = payload.len();
        if len < 126 {
            header.push(0x80 | len as u8);
        } else if len <= usize::from(u16::MAX) {
            header.push(0x80 | 126);
            header.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            header.push(0x80 | 127);
            header.extend_from_slice(&(len as u64).to_be_bytes());
        }
        let mask = rtd_crypto::random_bytes(4);
        header.extend_from_slice(&mask);
        let mut masked = Vec::with_capacity(len);
        for (index, byte) in payload.iter().enumerate() {
            masked.push(byte ^ mask[index % 4]);
        }
        self.io.write_all(&header)?;
        self.io.write_all(&masked)?;
        Ok(())
    }
}

impl<S: Read + Write> WebSocketIo<S> {
    /// Reads one frame, un-masking the payload.
    fn read_frame(&mut self) -> io::Result<(u8, Vec<u8>)> {
        let mut header = [0u8; 2];
        self.io.read_exact(&mut header)?;
        let opcode = header[0] & 0x0f;
        let masked = header[1] & 0x80 != 0;
        let short_len = usize::from(header[1] & 0x7f);
        let len = match short_len {
            126 => {
                let mut extended = [0u8; 2];
                self.io.read_exact(&mut extended)?;
                usize::from(u16::from_be_bytes(extended))
            }
            127 => {
                let mut extended = [0u8; 8];
                self.io.read_exact(&mut extended)?;
                usize::try_from(u64::from_be_bytes(extended))
                    .map_err(|_| io::Error::other("websocket frame length overflow"))?
            }
            other => other,
        };
        if len > MAX_FRAME {
            return Err(io::Error::other("websocket frame too large"));
        }
        let mask = if masked {
            let mut mask = [0u8; 4];
            self.io.read_exact(&mut mask)?;
            Some(mask)
        } else {
            None
        };
        let mut payload = vec![0u8; len];
        self.io.read_exact(&mut payload)?;
        if let Some(mask) = mask {
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }
        Ok((opcode, payload))
    }
}

impl<S: Read + Write> Read for WebSocketIo<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.buffer.len() {
            loop {
                let (opcode, payload) = self.read_frame()?;
                match opcode {
                    OPCODE_BINARY | OPCODE_TEXT | OPCODE_CONTINUATION => {
                        self.buffer = payload;
                        self.pos = 0;
                        if !self.buffer.is_empty() {
                            break;
                        }
                    }
                    OPCODE_CLOSE => return Ok(0),
                    OPCODE_PING => self.write_frame(OPCODE_PONG, &payload)?,
                    OPCODE_PONG => {}
                    other => {
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            format!("unexpected websocket opcode {other:#x}"),
                        ));
                    }
                }
            }
        }
        let available = self.buffer.len() - self.pos;
        let n = available.min(buf.len());
        buf[..n].copy_from_slice(&self.buffer[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl<S: Read + Write> Write for WebSocketIo<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_frame(OPCODE_BINARY, buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

/// Dials a `ws://` endpoint and returns a framed transport over the intermediate
/// codec.
///
/// `host` is sent in the HTTP `Host` header (for example `example.com:443`);
/// `path` is the WebSocket path (for example `/apiws`).
///
/// # Errors
///
/// Returns [`Error::Io`] when the socket cannot be created and
/// [`Error::Transport`] when the upgrade or framing setup fails.
pub fn connect_websocket(address: &str, host: &str, path: &str) -> Result<TcpTransport> {
    let inner = WebSocketIo::connect(address, host, path)?;
    Framed::new(
        Box::new(inner) as Box<dyn ReadWrite>,
        CodecKind::Intermediate,
        true,
    )
}

/// Encodes `data` as standard base64 (used for the handshake key).
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from);
        let b2 = chunk.get(2).copied().map(u32::from);
        let triple = (b0 << 16) | (b1.unwrap_or(0) << 8) | b2.unwrap_or(0);
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if b1.is_some() {
            ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if b2.is_some() {
            ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
