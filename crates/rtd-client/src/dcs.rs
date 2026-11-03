//! Datacenter discovery and resolution.
//!
//! [`StaticResolver`] holds a fixed DC list (production or test) and resolves a
//! datacenter identifier to an ordered list of endpoints and the transport each
//! uses.

use std::sync::OnceLock;

use rtd_crypto::rsa::RsaPublicKey;
use sha2::{Digest, Sha256};

/// The vendored Telegram DNS RSA public key, used to unwrap the encrypted
/// simple config published over DNS TXT records.
const DNS_PUBLIC_KEY: &str = "\
-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAyr+18Rex2ohtVy8sroGPBwXD3DOoKCSpjDqYoXgCqB7ioln4eDCF
fOBUlfXUEvM/fnKCpF46VkAftlb4VuPDeQSS/ZxZYEGqHaywlroVnXHIjgqoxiAd
192xRGreuXIaUKmkwlM9JID9WS2jUsTpzQ91L8MEPLJ/4zrBwZua8W5fECwCCh2c
9G5IzzBm+otMS/YKwmR1olzRCyEkyAEjXWqBI9Ftv5eG8m0VkBzOG655WIYdyV0H
fDK/NWcvGqa0w/nriMD6mDjKOryamw0OP9QuYgMN0C9xMW9y8SmP4h92OAWodTYg
Y1hZCxdv6cs5UnW9+PWvS+WIbkh+GaWYxwIDAQAB
-----END RSA PUBLIC KEY-----
";

/// The length of the encrypted DNS config in bytes.
const DNS_DECODED_LEN: usize = 256;
/// The length of the base64-encoded DNS config.
const DNS_ENCODED_LEN: usize = 344;

static DNS_KEY: OnceLock<Option<RsaPublicKey>> = OnceLock::new();

/// Returns the parsed vendored DNS key, if it could be parsed.
fn dns_key() -> Option<&'static RsaPublicKey> {
    DNS_KEY
        .get_or_init(|| rtd_crypto::parse_rsa_public_key(DNS_PUBLIC_KEY).ok())
        .as_ref()
}

/// A DNS configuration decoding failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsError(pub String);

impl core::fmt::Display for DnsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for DnsError {}

/// The Telegram DNS resolver configuration: datacenters with an optional expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsConfig {
    /// The `date` field of the simple config.
    pub date: i32,
    /// The `expires` field of the simple config.
    pub expires: i32,
    /// The datacenters the config describes.
    pub datacenters: Vec<DataCenter>,
}

impl DnsConfig {
    /// Creates a resolver over the decoded datacenters.
    #[must_use]
    pub fn resolver(self) -> StaticResolver {
        StaticResolver::new(self.datacenters)
    }
}

/// Decodes the encrypted simple config from a set of DNS TXT records.
///
/// The records are sorted by descending length, concatenated, base64-decoded,
/// unwrapped with the vendored RSA key and AES-CBC-decrypted, then parsed as a
/// `help.configSimple`.
///
/// # Errors
///
/// Returns [`DnsError`] when the records are malformed, base64 is invalid, the
/// hash does not match, or the plaintext is not a `help.configSimple`.
pub fn parse_dns_config(txt: &[String]) -> Result<DnsConfig, DnsError> {
    let mut parts: Vec<&str> = txt.iter().map(String::as_str).collect();
    parts.sort_by_key(|a| std::cmp::Reverse(a.len()));
    let total: usize = parts.iter().map(|part| part.len()).sum();
    if total != DNS_ENCODED_LEN {
        return Err(DnsError(format!("invalid input length {total}")));
    }
    let mut encoded = String::with_capacity(total);
    for part in parts {
        encoded.push_str(part);
    }
    let decoded =
        base64_decode(encoded.as_bytes()).ok_or_else(|| DnsError("invalid base64".to_owned()))?;
    if decoded.len() != DNS_DECODED_LEN {
        return Err(DnsError(format!(
            "invalid decoded length {}",
            decoded.len()
        )));
    }
    let mut input = [0u8; DNS_DECODED_LEN];
    input.copy_from_slice(&decoded);
    let config = decrypt_dns_config(&input)?;
    let datacenters = datacenters_from_rules(&config.rules);
    Ok(DnsConfig {
        date: config.date,
        expires: config.expires,
        datacenters,
    })
}

/// Unwraps and decodes one encrypted `help.configSimple` block.
fn decrypt_dns_config(input: &[u8; DNS_DECODED_LEN]) -> Result<rtd_tg::HelpConfigSimple, DnsError> {
    let key = dns_key().ok_or_else(|| DnsError("dns public key unavailable".to_owned()))?;
    let data_rsa = rtd_crypto::rsa_public_transform(key, input);

    let mut aes_key = [0u8; 32];
    aes_key.copy_from_slice(&data_rsa[..32]);
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&data_rsa[16..32]);

    let mut cbc = data_rsa[32..].to_vec();
    rtd_crypto::cbc_decrypt(&aes_key, &iv, &mut cbc);
    let split = cbc
        .len()
        .checked_sub(16)
        .ok_or_else(|| DnsError("cbc body too short".to_owned()))?;
    let (body, hash) = cbc.split_at(split);

    let digest = Sha256::digest(body);
    if digest[..16] != *hash {
        return Err(DnsError("hash mismatch".to_owned()));
    }
    if body.len() < 4 {
        return Err(DnsError("config body too short".to_owned()));
    }

    let mut decoder = rtd_tg::Decoder::new(&body[4..], rtd_tg::DecodeLimits::default());
    match rtd_tg::Object::decode(&mut decoder) {
        Ok(rtd_tg::Object::HelpConfigSimple(config)) => Ok(*config),
        Ok(other) => Err(DnsError(format!(
            "unexpected config constructor {:#010x}",
            other.type_id()
        ))),
        Err(error) => Err(DnsError(format!("decode config: {error}"))),
    }
}

/// Groups the config rules by datacenter, preserving endpoint order.
fn datacenters_from_rules(rules: &[rtd_tg::AccessPointRule]) -> Vec<DataCenter> {
    let mut datacenters: Vec<DataCenter> = Vec::new();
    for rule in rules {
        for ip in &rule.ips {
            let endpoint = match ip {
                rtd_tg::IpPort::IpPort { ipv4, port } => {
                    Endpoint::new(format!("{}:{}", ipv4_string(*ipv4), port), Protocol::Plain)
                }
                rtd_tg::IpPort::IpPortSecret { ipv4, port, secret } => {
                    let protocol = if secret.first() == Some(&0xee) {
                        Protocol::FakeTls
                    } else {
                        Protocol::Obfuscated2
                    };
                    Endpoint::with_secret(
                        format!("{}:{}", ipv4_string(*ipv4), port),
                        protocol,
                        secret.clone(),
                    )
                }
            };
            match datacenters.iter_mut().find(|dc| dc.id == rule.dc_id) {
                Some(dc) => dc.endpoints.push(endpoint),
                None => datacenters.push(DataCenter {
                    id: rule.dc_id,
                    endpoints: vec![endpoint],
                }),
            }
        }
    }
    datacenters
}

/// Formats a little-endian packed IPv4 as dotted decimal.
fn ipv4_string(ip: i32) -> String {
    let bytes = ip.to_le_bytes();
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}

/// Decodes standard base64 (padding optional).
fn base64_decode(input: &[u8]) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for &byte in input {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

/// The transport used to reach an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Plain TCP with the codec's framing.
    Plain,
    /// Obfuscated2 (MTProxy transport obfuscation).
    Obfuscated2,
    /// FakeTLS with obfuscated2 inside.
    FakeTls,
    /// MTProto over WebSocket.
    WebSocket,
}

/// A dialable endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// The `host:port` address.
    pub address: String,
    /// The transport to use.
    pub protocol: Protocol,
    /// The MTProxy secret, when the endpoint is obfuscated with one.
    pub secret: Option<Vec<u8>>,
}

impl Endpoint {
    /// Creates an endpoint.
    #[must_use]
    pub fn new(address: impl Into<String>, protocol: Protocol) -> Self {
        Self {
            address: address.into(),
            protocol,
            secret: None,
        }
    }

    /// Creates an endpoint carrying an MTProxy secret.
    #[must_use]
    pub fn with_secret(address: impl Into<String>, protocol: Protocol, secret: Vec<u8>) -> Self {
        Self {
            address: address.into(),
            protocol,
            secret: Some(secret),
        }
    }
}

/// A datacenter and its endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataCenter {
    /// The datacenter identifier.
    pub id: i32,
    /// The ordered endpoints to try.
    pub endpoints: Vec<Endpoint>,
}

impl DataCenter {
    /// Creates a datacenter with a single obfuscated2 endpoint.
    #[must_use]
    pub fn new(id: i32, address: impl Into<String>) -> Self {
        Self {
            id,
            endpoints: vec![Endpoint::new(address, Protocol::Obfuscated2)],
        }
    }
}

/// The migrate error kinds that carry a target datacenter in their argument.
const MIGRATE_KINDS: [&str; 5] = [
    "PHONE_MIGRATE",
    "NETWORK_MIGRATE",
    "USER_MIGRATE",
    "FILE_MIGRATE",
    "STATS_MIGRATE",
];

/// Returns the datacenter a `*_MIGRATE_<dc>` error points to, if any.
#[must_use]
pub fn migrate_target(error: &rtd_error::Error) -> Option<i32> {
    if MIGRATE_KINDS.contains(&error.kind()) {
        i32::try_from(error.argument()).ok()
    } else {
        None
    }
}

/// Resolves a datacenter identifier to its endpoints.
pub trait Resolver {
    /// Returns the datacenter for `id`, if known.
    fn resolve(&self, id: i32) -> Option<&DataCenter>;

    /// Returns every known datacenter.
    fn datacenters(&self) -> &[DataCenter];
}

/// A resolver backed by a fixed DC list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticResolver {
    datacenters: Vec<DataCenter>,
}

impl StaticResolver {
    /// Creates a resolver from an explicit DC list.
    #[must_use]
    pub fn new(datacenters: Vec<DataCenter>) -> Self {
        Self { datacenters }
    }

    /// Returns the Telegram production DC list.
    #[must_use]
    pub fn production() -> Self {
        Self::new(vec![
            DataCenter::new(1, "149.154.175.53:443"),
            DataCenter::new(2, "149.154.167.51:443"),
            DataCenter::new(3, "149.154.175.100:443"),
            DataCenter::new(4, "149.154.167.91:443"),
            DataCenter::new(5, "91.108.56.130:443"),
        ])
    }

    /// Returns the Telegram test DC list.
    #[must_use]
    pub fn test() -> Self {
        Self::new(vec![
            DataCenter::new(1, "149.154.175.10:443"),
            DataCenter::new(2, "149.154.167.40:443"),
            DataCenter::new(3, "149.154.175.117:443"),
        ])
    }

    /// Returns a copy with every endpoint using `protocol` and no secret.
    #[must_use]
    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        for datacenter in &mut self.datacenters {
            for endpoint in &mut datacenter.endpoints {
                endpoint.protocol = protocol;
                endpoint.secret = None;
            }
        }
        self
    }

    /// Builds a resolver routing every listed datacenter through one MTProxy.
    ///
    /// The endpoint protocol is derived from the secret's first byte: `0xee`
    /// selects FakeTLS, anything else Obfuscated2.
    #[must_use]
    pub fn mtproxy(
        datacenters: impl IntoIterator<Item = i32>,
        address: impl Into<String>,
        secret: Vec<u8>,
    ) -> Self {
        let protocol = if secret.first() == Some(&0xee) {
            Protocol::FakeTls
        } else {
            Protocol::Obfuscated2
        };
        let address = address.into();
        Self::new(
            datacenters
                .into_iter()
                .map(|id| DataCenter {
                    id,
                    endpoints: vec![Endpoint::with_secret(
                        address.clone(),
                        protocol,
                        secret.clone(),
                    )],
                })
                .collect(),
        )
    }

    /// Builds a WebSocket resolver from a per-datacenter domain list.
    #[must_use]
    pub fn websocket(domains: impl IntoIterator<Item = (i32, String)>) -> Self {
        Self::new(
            domains
                .into_iter()
                .map(|(id, address)| DataCenter {
                    id,
                    endpoints: vec![Endpoint::new(address, Protocol::WebSocket)],
                })
                .collect(),
        )
    }
}

impl Resolver for StaticResolver {
    fn resolve(&self, id: i32) -> Option<&DataCenter> {
        self.datacenters.iter().find(|dc| dc.id == id)
    }

    fn datacenters(&self) -> &[DataCenter] {
        &self.datacenters
    }
}

#[cfg(test)]
mod tests {
    use super::{Protocol, Resolver, StaticResolver};

    #[test]
    fn parses_migrate_errors() {
        use super::migrate_target;
        assert_eq!(
            migrate_target(&rtd_error::Error::new(303, "FILE_MIGRATE_2")),
            Some(2)
        );
        assert_eq!(
            migrate_target(&rtd_error::Error::new(400, "PHONE_MIGRATE_5")),
            Some(5)
        );
        assert_eq!(
            migrate_target(&rtd_error::Error::new(400, "FLOOD_WAIT_3")),
            None
        );
    }

    #[test]
    fn production_list_resolves() {
        let resolver = StaticResolver::production();
        let dc = resolver.resolve(2).unwrap();
        assert_eq!(dc.id, 2);
        assert_eq!(dc.endpoints.len(), 1);
        assert_eq!(dc.endpoints[0].protocol, Protocol::Obfuscated2);
        assert!(resolver.resolve(99).is_none());
        assert_eq!(resolver.datacenters().len(), 5);
    }

    fn dns_txt() -> Vec<String> {
        vec![
            "LcmEoukF2bVjKwz3E+J9BsDdL+rv9lGqLQWIGXrWACT2ESk5xuOpA6Cz6klKRbhbwSiHOd2zC5PiR57j/OJHPpj4i+tw==".to_owned(),
            concat!(
                "umjjLFLpOKtPeW9zHLq2ypbMzg/zkqvPhvhr0bxrLZlgPQ04l2GpO/4qZgAx3tk3BDHbY6/gmG1e8eaFBq3YSqR5SZ5hQ1Cm5f4/",
                "o67GYcPJClaf1TiHq3wVfsQ5OLnyJRw9A2ZfUfzIXxoSklPJrVdF/4hM1ZdUE0eWDAbmYf7JCeao8ecVVwKndd4CZHZS9wyf1T7DIUh95VpQ",
                "sn2klLPA6gA/2YNXOh9gITvjZrKuXLwwh9hBHhPvxv",
            )
            .to_owned(),
        ]
    }

    #[test]
    fn decodes_dns_config_vector() {
        use super::parse_dns_config;
        let config = parse_dns_config(&dns_txt()).unwrap();
        assert_eq!(config.date, 1_562_949_126);
        assert_eq!(config.expires, 1_565_541_126);
        assert_eq!(config.datacenters.len(), 1);
        let dc = &config.datacenters[0];
        assert_eq!(dc.id, 2);
        assert_eq!(dc.endpoints.len(), 1);
        assert_eq!(dc.endpoints[0].address, "98.210.59.139:14544");
        assert_eq!(dc.endpoints[0].protocol, Protocol::FakeTls);
        assert!(dc.endpoints[0].secret.is_some());
    }

    #[test]
    fn rejects_bad_dns_input() {
        use super::parse_dns_config;
        assert!(parse_dns_config(&[]).is_err());
        let mut broken = dns_txt();
        broken[0] = format!("#{}", &broken[0][1..]);
        assert!(parse_dns_config(&broken).is_err());
    }

    #[test]
    fn constructs_plain_mtproxy_and_websocket_resolvers() {
        let plain = StaticResolver::production().with_protocol(Protocol::Plain);
        assert_eq!(
            plain.resolve(1).unwrap().endpoints[0].protocol,
            Protocol::Plain
        );
        assert!(plain.resolve(1).unwrap().endpoints[0].secret.is_none());

        let proxy = StaticResolver::mtproxy([1, 2], "proxy.example:443", vec![0xee, 0, 0, 0]);
        let endpoint = &proxy.resolve(2).unwrap().endpoints[0];
        assert_eq!(endpoint.address, "proxy.example:443");
        assert_eq!(endpoint.protocol, Protocol::FakeTls);
        assert_eq!(endpoint.secret.as_deref(), Some([0xee, 0, 0, 0].as_slice()));

        let websocket = StaticResolver::websocket([(2, "wss://example/apiws".to_owned())]);
        assert_eq!(
            websocket.resolve(2).unwrap().endpoints[0].protocol,
            Protocol::WebSocket
        );
    }
}
