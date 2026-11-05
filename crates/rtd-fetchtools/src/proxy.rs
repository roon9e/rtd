//! A minimal SOCKS5 client for `giftfetch`'s `SOCKS5_PROXY` egress.
//!
//! `rtd-client`'s `dcs` layer covers MTProxy/FakeTLS/WebSocket but not SOCKS5,
//! so the fetchers dial the proxy themselves and then run the standard
//! obfuscated2 handshake over the resulting stream (see
//! [`crate::client::Tg`]).

use std::io::{Read, Write};
use std::net::TcpStream;

use crate::Result;

/// A parsed SOCKS5 proxy endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proxy {
    /// `host:port`.
    pub address: String,
    /// Optional username/password authentication.
    pub auth: Option<(String, String)>,
}

/// Parses a `SOCKS5_PROXY` value: `host:port` or `socks5://[user:pass@]host:port`.
///
/// # Errors
///
/// Returns an error for an unsupported scheme, a missing host/port, or an
/// unexpected path, query or fragment.
pub fn parse(raw: &str) -> Result<Proxy> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("must not be empty".into());
    }
    if raw.contains("://") {
        let Some(rest) = raw.strip_prefix("socks5://") else {
            return Err("scheme must be socks5".into());
        };
        let (auth, host_port) = match rest.split_once('@') {
            Some((userinfo, host_port)) => (Some(userinfo), host_port),
            None => (None, rest),
        };
        if host_port.contains('/') || host_port.contains('?') || host_port.contains('#') {
            return Err("must not include path, query, or fragment".into());
        }
        let address = normalize_address(host_port)?;
        let auth = auth.map(|userinfo| {
            let (user, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));
            (user.to_owned(), password.to_owned())
        });
        return Ok(Proxy { address, auth });
    }
    let address = normalize_address(raw)?;
    Ok(Proxy {
        address,
        auth: None,
    })
}

/// Validates and normalizes a `host:port` pair.
fn normalize_address(raw: &str) -> Result<String> {
    let raw = raw.trim();
    let (host, port) = raw
        .rsplit_once(':')
        .ok_or("must be host:port or socks5://host:port")?;
    if host.is_empty() || port.is_empty() {
        return Err("host and port are required".into());
    }
    Ok(format!("{host}:{port}"))
}

/// Connects to `target` through the SOCKS5 `proxy`, returning the ready stream.
///
/// # Errors
///
/// Returns an I/O error when the proxy cannot be reached or rejects the request.
pub fn connect(target: &str, proxy: &Proxy) -> std::io::Result<TcpStream> {
    let mut stream = TcpStream::connect(&proxy.address)?;
    stream.set_nodelay(true)?;
    handshake(&mut stream, target, proxy)?;
    Ok(stream)
}

/// Runs the SOCKS5 greeting, authentication and connect request.
fn handshake(stream: &mut TcpStream, target: &str, proxy: &Proxy) -> std::io::Result<()> {
    // Greeting: offer no-auth, and username/password when configured.
    let methods: &[u8] = if proxy.auth.is_some() {
        &[0x00, 0x02]
    } else {
        &[0x00]
    };
    let mut greeting = Vec::with_capacity(2 + methods.len());
    greeting.push(0x05);
    greeting.push(methods.len() as u8);
    greeting.extend_from_slice(methods);
    stream.write_all(&greeting)?;

    let mut choice = [0u8; 2];
    stream.read_exact(&mut choice)?;
    if choice[0] != 0x05 {
        return Err(invalid("proxy did not answer with SOCKS5"));
    }
    match choice[1] {
        0x00 => {}
        0x02 => {
            let Some((user, password)) = &proxy.auth else {
                return Err(invalid("proxy requires authentication"));
            };
            if user.len() > 255 || password.len() > 255 {
                return Err(invalid("proxy credentials are too long"));
            }
            let mut request = Vec::with_capacity(3 + user.len() + password.len());
            request.push(0x01);
            request.push(user.len() as u8);
            request.extend_from_slice(user.as_bytes());
            request.push(password.len() as u8);
            request.extend_from_slice(password.as_bytes());
            stream.write_all(&request)?;
            let mut reply = [0u8; 2];
            stream.read_exact(&mut reply)?;
            if reply[1] != 0x00 {
                return Err(invalid("proxy rejected the credentials"));
            }
        }
        0xff => return Err(invalid("proxy accepted no authentication method")),
        _ => return Err(invalid("proxy selected an unsupported method")),
    }

    // CONNECT request with a domain-name target.
    let (host, port) = target
        .rsplit_once(':')
        .ok_or_else(|| invalid("target must be host:port"))?;
    let port: u16 = port
        .parse()
        .map_err(|_| invalid("target port is not a number"))?;
    if host.len() > 255 {
        return Err(invalid("target host is too long"));
    }
    let mut request = Vec::with_capacity(7 + host.len());
    request.push(0x05);
    request.push(0x01);
    request.push(0x00);
    request.push(0x03);
    request.push(host.len() as u8);
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request)?;

    // Reply: version, status, reserved, address type, address, port.
    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    if header[1] != 0x00 {
        return Err(invalid("proxy refused the connection"));
    }
    let address_len = match header[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len)?;
            usize::from(len[0])
        }
        _ => return Err(invalid("proxy replied with an unknown address type")),
    };
    let mut tail = vec![0u8; address_len + 2];
    stream.read_exact(&mut tail)?;
    Ok(())
}

/// Builds an invalid-data I/O error.
fn invalid(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_host_port_and_url_forms() {
        assert_eq!(parse("127.0.0.1:1080").unwrap().address, "127.0.0.1:1080");
        let with_auth = parse("socks5://user:pass@proxy.example:1080").unwrap();
        assert_eq!(with_auth.address, "proxy.example:1080");
        assert_eq!(with_auth.auth, Some(("user".to_owned(), "pass".to_owned())));
    }

    #[test]
    fn rejects_bad_proxy_strings() {
        for bad in ["", "socks5://host:1080/path", "host", "http://host:1"] {
            assert!(parse(bad).is_err(), "{bad} should be rejected");
        }
    }
}
