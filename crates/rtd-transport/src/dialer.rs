//! MTProxy-aware dialing.
//!
//! [`DefaultDialer`] connects to an endpoint and applies the obfuscation layer
//! selected by a parsed [`Secret`]: obfuscated2 for a plain or secured (`dd`)
//! secret, and FakeTLS-over-obfuscated2 for a FakeTLS (`ee`) secret, matching
//! the reference `mtproxy/obfuscator` composition.

use std::io::{Read, Write};
use std::net::TcpStream;

use crate::error::Result;
use crate::faketls::FakeTls;
use crate::obfuscated2::Obfuscated2;
use crate::secret::{Secret, SecretMode};

/// A connected byte stream, plain or obfuscated.
pub trait Stream: Read + Write + Send {}

impl<T: Read + Write + Send> Stream for T {}

/// Connects to an endpoint and applies the obfuscation selected by a secret.
pub trait Dialer {
    /// Connects to `address` for datacenter `dc` with the transport `tag`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Io`] when the socket cannot be created, and a
    /// handshake error when the obfuscation layer fails.
    fn dial(
        &self,
        address: &str,
        dc: u16,
        tag: [u8; 4],
        secret: &Secret,
    ) -> Result<Box<dyn Stream>>;
}

/// The default dialer: TCP with obfuscated2, or FakeTLS-over-obfuscated2 for `ee`.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultDialer;

impl Dialer for DefaultDialer {
    fn dial(
        &self,
        address: &str,
        dc: u16,
        tag: [u8; 4],
        secret: &Secret,
    ) -> Result<Box<dyn Stream>> {
        let socket = TcpStream::connect(address)?;
        socket.set_nodelay(true)?;
        match secret.mode() {
            SecretMode::Plain | SecretMode::Secured => {
                let key = secret.obfuscated2_key()?;
                Ok(Box::new(Obfuscated2::handshake(socket, tag, dc, key)?))
            }
            SecretMode::FakeTls => {
                let key = secret.faketls_key()?;
                let mut tls = FakeTls::new(socket);
                tls.handshake(key, secret.cloak_host())?;
                Ok(Box::new(Obfuscated2::handshake(tls, tag, dc, key)?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::{DefaultDialer, Dialer};
    use crate::obfuscated2::Obfuscated2;
    use crate::secret::Secret;

    #[test]
    fn plain_dial_composes_with_obfuscated2() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let secret = Secret::parse("52a493bdfb90eea55739eabff2d92a14").unwrap();
        let key = secret.obfuscated2_key().unwrap().to_vec();

        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let (mut stream, meta) = Obfuscated2::accept(socket, &key).unwrap();
            let mut buf = [0u8; 5];
            stream.read_exact(&mut buf).unwrap();
            (meta.dc, buf)
        });

        let mut client = DefaultDialer
            .dial(&addr.to_string(), 2, [0xef; 4], &secret)
            .unwrap();
        client.write_all(b"hello").unwrap();
        client.flush().unwrap();

        let (dc, buf) = server.join().unwrap();
        assert_eq!(dc, 2);
        assert_eq!(&buf, b"hello");
    }
}
