//! FakeTLS framing used by MTProxy.
//!
//! FakeTLS wraps the obfuscated2 stream in TLS records so that the connection
//! looks like an ordinary TLS session. The handshake carries an HMAC-SHA256
//! digest in the TLS random field, keyed by the proxy secret, which the peer
//! verifies to authenticate the session.
//!
//! See <https://core.telegram.org/mtproto/mtproto-transports#transport-obfuscation>.

use crate::error::{Error, Result};
use sha2::Sha256;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

/// TLS `ChangeCipherSpec` record type.
pub const RECORD_TYPE_CHANGE_CIPHER_SPEC: u8 = 0x14;
/// TLS `Alert` record type.
pub const RECORD_TYPE_ALERT: u8 = 0x15;
/// TLS `Handshake` record type.
pub const RECORD_TYPE_HANDSHAKE: u8 = 0x16;
/// TLS `Application` record type.
pub const RECORD_TYPE_APPLICATION: u8 = 0x17;
/// TLS `Heartbeat` record type.
pub const RECORD_TYPE_HEARTBEAT: u8 = 0x18;

/// TLS 1.0 protocol version.
pub const VERSION_10: [u8; 2] = [0x03, 0x01];
/// TLS 1.1 protocol version.
pub const VERSION_11: [u8; 2] = [0x03, 0x02];
/// TLS 1.2 protocol version.
pub const VERSION_12: [u8; 2] = [0x03, 0x03];
/// TLS 1.3 (legacy) protocol version.
pub const VERSION_13: [u8; 2] = [0x03, 0x04];

/// Offset of the 32-byte random field within a handshake record.
pub const RANDOM_OFFSET: usize = 11;
/// Length of the TLS random field.
pub const RANDOM_LENGTH: usize = 32;

/// Maximum number of extra handshake records accepted before ChangeCipherSpec.
pub const MAX_HANDSHAKE_RECORDS: usize = 16;

/// A single TLS record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Record type byte.
    pub record_type: u8,
    /// Protocol version.
    pub version: [u8; 2],
    /// Record payload.
    pub data: Vec<u8>,
}

impl Record {
    /// Creates a record from its parts.
    #[must_use]
    pub const fn new(record_type: u8, version: [u8; 2], data: Vec<u8>) -> Self {
        Self {
            record_type,
            version,
            data,
        }
    }
}

/// Writes a TLS record.
///
/// # Errors
///
/// Returns an error if the payload does not fit in the 16-bit length field or
/// the underlying writer fails.
pub fn write_record<W: Write>(w: &mut W, record: &Record) -> Result<()> {
    let length =
        u16::try_from(record.data.len()).map_err(|_| Error::RecordTooLarge(record.data.len()))?;
    let header = [
        record.record_type,
        record.version[0],
        record.version[1],
        (length >> 8) as u8,
        (length & 0xff) as u8,
    ];
    w.write_all(&header)?;
    w.write_all(&record.data)?;
    Ok(())
}

/// Reads a TLS record.
///
/// # Errors
///
/// Returns an error if the underlying reader fails or the version is unknown.
pub fn read_record<R: Read>(r: &mut R) -> Result<Record> {
    let mut header = [0u8; 5];
    r.read_exact(&mut header)?;

    let version = [header[1], header[2]];
    match version {
        VERSION_10 | VERSION_11 | VERSION_12 | VERSION_13 => {}
        other => return Err(Error::UnknownProtocolVersion(other)),
    }

    let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
    let mut data = vec![0u8; length];
    r.read_exact(&mut data)?;

    Ok(Record {
        record_type: header[0],
        version,
        data,
    })
}

/// Appends the wire encoding of `record` to `out`.
fn encode_record(out: &mut Vec<u8>, record: &Record) {
    out.push(record.record_type);
    out.extend_from_slice(&record.version);
    let length = record.data.len() as u16;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(&record.data);
}

/// Computes `HMAC-SHA256(key, data)`.
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    use hmac::digest::KeyInit;
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts a key of any length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// Builds a minimal, well-formed TLS ClientHello record carrying `domain`.
///
/// # Errors
///
/// Returns an error if the random source fails.
pub fn generate_client_hello(domain: &str) -> Result<Vec<u8>> {
    let mut random = [0u8; RANDOM_LENGTH];
    let mut session_id = [0u8; 32];
    getrandom::fill(&mut random).map_err(|err| Error::Io(std::io::Error::other(err)))?;
    getrandom::fill(&mut session_id).map_err(|err| Error::Io(std::io::Error::other(err)))?;

    let mut body = Vec::new();
    // legacy_version (TLS 1.2).
    body.extend_from_slice(&VERSION_12);
    // random (placeholder, filled by the caller).
    body.extend_from_slice(&random);
    // session_id: 32 bytes, echoed by the server.
    body.push(0x20);
    body.extend_from_slice(&session_id);
    // cipher_suites: TLS 1.3 suites first, then a couple of TLS 1.2 suites.
    let suites: [u8; 10] = [0x13, 0x01, 0x13, 0x02, 0x13, 0x03, 0xc0, 0x2b, 0xc0, 0x2f];
    body.extend_from_slice(&(suites.len() as u16).to_be_bytes());
    body.extend_from_slice(&suites);
    // compression_methods: null.
    body.push(1);
    body.push(0);

    // extensions.
    let mut extensions = Vec::new();
    let name = domain.as_bytes();
    // server_name (SNI).
    extensions.extend_from_slice(&0u16.to_be_bytes());
    let sni_list_len = 1 + 2 + name.len();
    extensions.extend_from_slice(&((2 + sni_list_len) as u16).to_be_bytes());
    extensions.extend_from_slice(&(sni_list_len as u16).to_be_bytes());
    extensions.push(0x00);
    extensions.extend_from_slice(&(name.len() as u16).to_be_bytes());
    extensions.extend_from_slice(name);
    // supported_versions: TLS 1.3.
    extensions.extend_from_slice(&0x002b_u16.to_be_bytes());
    extensions.extend_from_slice(&3u16.to_be_bytes());
    extensions.push(2);
    extensions.extend_from_slice(&VERSION_13);
    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend_from_slice(&extensions);

    // Handshake header + record header.
    let mut handshake = Vec::new();
    handshake.push(0x01); // ClientHello
    let body_len = body.len();
    handshake.extend_from_slice(&[
        (body_len >> 16) as u8,
        (body_len >> 8) as u8,
        body_len as u8,
    ]);
    handshake.extend_from_slice(&body);

    let mut record = Vec::new();
    record.push(RECORD_TYPE_HANDSHAKE);
    record.extend_from_slice(&VERSION_10);
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    Ok(record)
}

/// Builds and writes a ClientHello, returning the ClientRandom used for the
/// digest.
///
/// The random field is set to `HMAC-SHA256(secret, record)` computed with the
/// random zeroed; its final four bytes are XORed with `now_unix`.
///
/// # Errors
///
/// Returns an error if the random source or the writer fails.
pub fn write_client_hello<W: Write>(
    w: &mut W,
    now_unix: u32,
    domain: &str,
    secret: &[u8],
) -> Result<[u8; RANDOM_LENGTH]> {
    let mut record = generate_client_hello(domain)?;
    let end = RANDOM_OFFSET + RANDOM_LENGTH;
    if record.len() < end {
        return Err(Error::ClientHelloTooShort(record.len()));
    }

    record[RANDOM_OFFSET..end].fill(0);
    let digest = hmac_sha256(secret, &record);

    let mut client_random = [0u8; RANDOM_LENGTH];
    client_random.copy_from_slice(&digest);
    let mut tail = [0u8; 4];
    tail.copy_from_slice(&client_random[RANDOM_LENGTH - 4..]);
    let timestamp = u32::from_le_bytes(tail) ^ now_unix;
    client_random[RANDOM_LENGTH - 4..].copy_from_slice(&timestamp.to_le_bytes());

    record[RANDOM_OFFSET..end].copy_from_slice(&client_random);

    w.write_all(&record)?;
    Ok(client_random)
}

/// Verifies a received ClientHello and returns its ClientRandom.
///
/// The first 28 bytes of the random must equal `HMAC-SHA256(secret, record)`
/// with the random zeroed.
///
/// # Errors
///
/// Returns an error if the record type is wrong, the record is too short, or
/// the digest does not match.
pub fn verify_client_hello(record: &[u8], secret: &[u8]) -> Result<[u8; RANDOM_LENGTH]> {
    let end = RANDOM_OFFSET + RANDOM_LENGTH;
    if record.len() < end {
        return Err(Error::ClientHelloTooShort(record.len()));
    }
    if record[0] != RECORD_TYPE_HANDSHAKE {
        return Err(Error::UnexpectedRecordType {
            expected: "handshake",
            actual: record[0],
        });
    }

    let mut random = [0u8; RANDOM_LENGTH];
    random.copy_from_slice(&record[RANDOM_OFFSET..end]);

    let mut zeroed = record.to_vec();
    zeroed[RANDOM_OFFSET..end].fill(0);
    let digest = hmac_sha256(secret, &zeroed);
    if digest[..RANDOM_LENGTH - 4] != random[..RANDOM_LENGTH - 4] {
        return Err(Error::HmacMismatch);
    }
    Ok(random)
}

/// Builds and writes a ServerHello response that `read_server_hello` accepts.
///
/// The response is a ServerHello handshake record followed by a
/// ChangeCipherSpec record and an Application record. Its random field carries
/// `HMAC-SHA256(secret, client_random || packet)` computed with the random
/// zeroed.
///
/// # Errors
///
/// Returns an error if the writer fails.
pub fn write_server_hello<W: Write>(
    w: &mut W,
    client_random: [u8; RANDOM_LENGTH],
    secret: &[u8],
) -> Result<()> {
    // ServerHello handshake record.
    let mut body = Vec::new();
    body.extend_from_slice(&VERSION_12);
    body.extend_from_slice(&[0u8; RANDOM_LENGTH]);
    body.push(0x20);
    body.extend_from_slice(&[0u8; 32]);
    body.extend_from_slice(&[0x13, 0x01]);
    body.push(0x00);

    let mut handshake = Vec::new();
    handshake.push(0x02); // ServerHello
    let body_len = body.len();
    handshake.extend_from_slice(&[
        (body_len >> 16) as u8,
        (body_len >> 8) as u8,
        body_len as u8,
    ]);
    handshake.extend_from_slice(&body);

    let mut packet = Vec::new();
    encode_record(
        &mut packet,
        &Record::new(RECORD_TYPE_HANDSHAKE, VERSION_12, handshake),
    );
    encode_record(
        &mut packet,
        &Record::new(RECORD_TYPE_CHANGE_CIPHER_SPEC, VERSION_12, vec![0x01]),
    );
    encode_record(
        &mut packet,
        &Record::new(RECORD_TYPE_APPLICATION, VERSION_12, vec![0, 0, 0, 0]),
    );

    let end = RANDOM_OFFSET + RANDOM_LENGTH;
    if packet.len() < end {
        return Err(Error::HandshakeRecordTooShort);
    }
    packet[RANDOM_OFFSET..end].fill(0);

    let mut mac_input = Vec::with_capacity(RANDOM_LENGTH + packet.len());
    mac_input.extend_from_slice(&client_random);
    mac_input.extend_from_slice(&packet);
    let digest = hmac_sha256(secret, &mac_input);
    packet[RANDOM_OFFSET..end].copy_from_slice(&digest);

    w.write_all(&packet)?;
    Ok(())
}

/// Reads and verifies a FakeTLS ServerHello response.
///
/// At most [`MAX_HANDSHAKE_RECORDS`] extra handshake records may precede the
/// ChangeCipherSpec record, which must be followed by an Application record.
///
/// # Errors
///
/// Returns an error if a record has an unexpected type, no ChangeCipherSpec is
/// seen, the first record is too short, or the digest does not match.
pub fn read_server_hello<R: Read>(
    r: &mut R,
    client_random: [u8; RANDOM_LENGTH],
    secret: &[u8],
) -> Result<()> {
    let mut packet = Vec::new();

    let first = read_record(r)?;
    if first.record_type != RECORD_TYPE_HANDSHAKE {
        return Err(Error::UnexpectedRecordType {
            expected: "handshake",
            actual: first.record_type,
        });
    }
    encode_record(&mut packet, &first);
    if packet.len() < RANDOM_OFFSET + RANDOM_LENGTH {
        return Err(Error::HandshakeRecordTooShort);
    }

    let mut change_cipher_found = false;
    for _ in 0..MAX_HANDSHAKE_RECORDS {
        let record = read_record(r)?;
        encode_record(&mut packet, &record);
        match record.record_type {
            RECORD_TYPE_HANDSHAKE => continue,
            RECORD_TYPE_CHANGE_CIPHER_SPEC => change_cipher_found = true,
            _ => {
                return Err(Error::UnexpectedRecordType {
                    expected: "handshake or change_cipher_spec",
                    actual: record.record_type,
                });
            }
        }
        break;
    }
    if !change_cipher_found {
        return Err(Error::MissingChangeCipherSpec);
    }

    let cert = read_record(r)?;
    encode_record(&mut packet, &cert);
    if cert.record_type != RECORD_TYPE_APPLICATION {
        return Err(Error::UnexpectedRecordType {
            expected: "application",
            actual: cert.record_type,
        });
    }

    let end = RANDOM_OFFSET + RANDOM_LENGTH;
    let mut original = [0u8; RANDOM_LENGTH];
    original.copy_from_slice(&packet[RANDOM_OFFSET..end]);
    packet[RANDOM_OFFSET..end].fill(0);

    let mut mac_input = Vec::with_capacity(RANDOM_LENGTH + packet.len());
    mac_input.extend_from_slice(&client_random);
    mac_input.extend_from_slice(&packet);
    let digest = hmac_sha256(secret, &mac_input);
    if digest != original {
        return Err(Error::HmacMismatch);
    }
    Ok(())
}

/// Runs the client-side FakeTLS handshake over `conn`.
///
/// # Errors
///
/// Returns an error if the ClientHello cannot be written or the ServerHello
/// cannot be verified.
pub fn client_handshake<S: Read + Write>(conn: &mut S, secret: &[u8], domain: &str) -> Result<()> {
    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as u32,
        Err(_) => 0,
    };
    let client_random = write_client_hello(conn, now, domain, secret)?;
    read_server_hello(conn, client_random, secret)
}

/// A FakeTLS record stream wrapping an underlying connection.
pub struct FakeTls<S> {
    conn: S,
    version: [u8; 2],
    first_packet: bool,
    read_buf: Vec<u8>,
    read_pos: usize,
}

impl<S> FakeTls<S> {
    /// Wraps `conn` in FakeTLS record framing.
    #[must_use]
    pub const fn new(conn: S) -> Self {
        Self {
            conn,
            version: VERSION_12,
            first_packet: false,
            read_buf: Vec::new(),
            read_pos: 0,
        }
    }

    /// Returns a reference to the underlying connection.
    #[must_use]
    pub const fn get_ref(&self) -> &S {
        &self.conn
    }

    /// Returns a mutable reference to the underlying connection.
    #[must_use]
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.conn
    }

    /// Consumes the stream and returns the underlying connection.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.conn
    }
}

impl<S: Read + Write> FakeTls<S> {
    /// Runs the client FakeTLS handshake over the underlying connection.
    ///
    /// # Errors
    ///
    /// See [`client_handshake`].
    pub fn handshake(&mut self, secret: &[u8], domain: &str) -> Result<()> {
        self.read_buf.clear();
        self.read_pos = 0;
        client_handshake(&mut self.conn, secret, domain)
    }
}

impl<S: Read + Write> Read for FakeTls<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.read_pos < self.read_buf.len() {
                let available = self.read_buf.len() - self.read_pos;
                let n = available.min(buf.len());
                buf[..n].copy_from_slice(&self.read_buf[self.read_pos..self.read_pos + n]);
                self.read_pos += n;
                if self.read_pos == self.read_buf.len() {
                    self.read_buf.clear();
                    self.read_pos = 0;
                }
                return Ok(n);
            }

            let record = read_record(&mut self.conn).map_err(std::io::Error::other)?;
            match record.record_type {
                RECORD_TYPE_CHANGE_CIPHER_SPEC => continue,
                RECORD_TYPE_APPLICATION => {}
                RECORD_TYPE_HANDSHAKE => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "unexpected record type handshake",
                    ));
                }
                other => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("unsupported record type {other}"),
                    ));
                }
            }
            self.read_buf = record.data;
            self.read_pos = 0;
        }
    }
}

impl<S: Read + Write> Write for FakeTls<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !self.first_packet {
            write_record(
                &mut self.conn,
                &Record::new(RECORD_TYPE_CHANGE_CIPHER_SPEC, self.version, vec![0x01]),
            )
            .map_err(std::io::Error::other)?;
            self.first_packet = true;
        }
        write_record(
            &mut self.conn,
            &Record::new(RECORD_TYPE_APPLICATION, self.version, buf.to_vec()),
        )
        .map_err(std::io::Error::other)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.conn.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_HANDSHAKE_RECORDS, RANDOM_LENGTH, RANDOM_OFFSET, RECORD_TYPE_APPLICATION,
        RECORD_TYPE_CHANGE_CIPHER_SPEC, RECORD_TYPE_HANDSHAKE, Record, VERSION_10, VERSION_12,
        client_handshake, generate_client_hello, read_record, read_server_hello,
        verify_client_hello, write_client_hello, write_record, write_server_hello,
    };
    use crate::faketls::FakeTls;
    use std::io::{Cursor, Read, Write};

    const SECRET: &[u8] = b"0123456789abcdef";

    #[test]
    fn record_round_trip() {
        let record = Record::new(RECORD_TYPE_APPLICATION, VERSION_10, b"abcd".to_vec());
        let mut buffer = Vec::new();
        write_record(&mut buffer, &record).unwrap();
        assert_eq!(
            buffer,
            vec![0x17, 0x03, 0x01, 0x00, 0x04, b'a', b'b', b'c', b'd']
        );
        let decoded = read_record(&mut Cursor::new(buffer)).unwrap();
        assert_eq!(decoded, record);
    }

    #[test]
    fn record_rejects_unknown_version() {
        let mut buffer = vec![0x17, 0x09, 0x09, 0x00, 0x00];
        buffer.extend_from_slice(&[]);
        assert!(read_record(&mut Cursor::new(buffer)).is_err());
    }

    #[test]
    fn client_hello_has_expected_layout() {
        let record = generate_client_hello("example.com").unwrap();
        assert_eq!(record[0], RECORD_TYPE_HANDSHAKE);
        assert_eq!(&record[1..3], &VERSION_10);
        // Record length matches the body.
        let record_len = usize::from(u16::from_be_bytes([record[3], record[4]]));
        assert_eq!(record_len, record.len() - 5);
        // Handshake type and version.
        assert_eq!(record[5], 0x01);
        assert_eq!(&record[9..11], &VERSION_12);
        // Session id length at offset 43.
        assert_eq!(record[43], 0x20);
        // SNI carries the domain.
        assert!(record.windows(11).any(|w| w == b"example.com"));
        // Cipher suites begin at offset 76.
        let suites_len = usize::from(u16::from_be_bytes([record[76], record[77]]));
        assert_eq!(&record[78..80], &[0x13, 0x01]);
        assert!(suites_len >= 2);
    }

    #[test]
    fn client_hello_verifies() {
        let mut record = Vec::new();
        let random = write_client_hello(&mut record, 1_700_000_000, "example.com", SECRET).unwrap();
        let verified = verify_client_hello(&record, SECRET).unwrap();
        assert_eq!(verified, random);
        // The digest is bound to the secret.
        assert!(verify_client_hello(&record, b"wrongsecretvalue").is_err());
    }

    #[test]
    fn client_hello_is_randomized() {
        let first = generate_client_hello("example.com").unwrap();
        let second = generate_client_hello("example.com").unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn server_hello_round_trip() {
        let mut hello = Vec::new();
        let client_random =
            write_client_hello(&mut hello, 1_700_000_000, "example.com", SECRET).unwrap();

        let mut response = Vec::new();
        write_server_hello(&mut response, client_random, SECRET).unwrap();

        read_server_hello(&mut Cursor::new(response.clone()), client_random, SECRET).unwrap();
        // A different client random fails verification.
        let other = [0u8; RANDOM_LENGTH];
        assert!(read_server_hello(&mut Cursor::new(response), other, SECRET).is_err());
    }

    #[test]
    fn server_hello_allows_extra_handshake_records() {
        let client_random = [0u8; RANDOM_LENGTH];
        let mut packet = Vec::new();
        // One ServerHello-like handshake record, then MAX extra handshake
        // records, then ChangeCipherSpec and Application.
        let mut body = vec![0u8; 38];
        body[0] = 0x03;
        body[1] = 0x03;
        let mut handshake = vec![0x02];
        handshake.extend_from_slice(&[0, 0, body.len() as u8]);
        handshake.extend_from_slice(&body);
        write_record(
            &mut packet,
            &Record::new(RECORD_TYPE_HANDSHAKE, VERSION_12, handshake),
        )
        .unwrap();
        for _ in 0..(MAX_HANDSHAKE_RECORDS - 1) {
            write_record(
                &mut packet,
                &Record::new(RECORD_TYPE_HANDSHAKE, VERSION_12, vec![0x0b, 0, 0, 0]),
            )
            .unwrap();
        }
        write_record(
            &mut packet,
            &Record::new(RECORD_TYPE_CHANGE_CIPHER_SPEC, VERSION_12, vec![0x01]),
        )
        .unwrap();
        write_record(
            &mut packet,
            &Record::new(RECORD_TYPE_APPLICATION, VERSION_12, vec![0x14, 0, 0, 0]),
        )
        .unwrap();

        // Compute and place the digest over the packet with the random zeroed.
        let end = RANDOM_OFFSET + RANDOM_LENGTH;
        packet[RANDOM_OFFSET..end].fill(0);
        let mut mac_input = Vec::new();
        mac_input.extend_from_slice(&client_random);
        mac_input.extend_from_slice(&packet);
        let digest = super::hmac_sha256(SECRET, &mac_input);
        packet[RANDOM_OFFSET..end].copy_from_slice(&digest);

        read_server_hello(&mut Cursor::new(packet), client_random, SECRET).unwrap();
    }

    #[test]
    fn server_hello_error_paths() {
        let client_random = [0u8; RANDOM_LENGTH];
        let secret = SECRET;

        // First record is not a handshake.
        let mut buffer = Vec::new();
        write_record(
            &mut buffer,
            &Record::new(RECORD_TYPE_APPLICATION, VERSION_12, vec![1]),
        )
        .unwrap();
        assert!(read_server_hello(&mut Cursor::new(buffer), client_random, secret).is_err());

        // First handshake too short.
        let mut buffer = Vec::new();
        write_record(
            &mut buffer,
            &Record::new(RECORD_TYPE_HANDSHAKE, VERSION_12, vec![1]),
        )
        .unwrap();
        assert!(read_server_hello(&mut Cursor::new(buffer), client_random, secret).is_err());

        // Unexpected record before ChangeCipherSpec.
        let mut buffer = Vec::new();
        write_record(
            &mut buffer,
            &Record::new(RECORD_TYPE_HANDSHAKE, VERSION_12, vec![0u8; 38]),
        )
        .unwrap();
        write_record(
            &mut buffer,
            &Record::new(RECORD_TYPE_APPLICATION, VERSION_12, vec![1]),
        )
        .unwrap();
        assert!(read_server_hello(&mut Cursor::new(buffer), client_random, secret).is_err());
    }

    #[test]
    fn fake_tls_read_skips_change_cipher_spec() {
        let mut conn = Vec::new();
        write_record(
            &mut conn,
            &Record::new(RECORD_TYPE_CHANGE_CIPHER_SPEC, VERSION_12, vec![0x01]),
        )
        .unwrap();
        write_record(
            &mut conn,
            &Record::new(RECORD_TYPE_APPLICATION, VERSION_12, b"hello".to_vec()),
        )
        .unwrap();

        let mut stream = FakeTls::new(Cursor::new(conn));
        let mut decoded = [0u8; 5];
        stream.read_exact(&mut decoded).unwrap();
        assert_eq!(&decoded, b"hello");
    }

    #[test]
    fn fake_tls_rejects_unexpected_records() {
        let mut conn = Vec::new();
        write_record(
            &mut conn,
            &Record::new(RECORD_TYPE_HANDSHAKE, VERSION_12, vec![1]),
        )
        .unwrap();
        let mut stream = FakeTls::new(Cursor::new(conn));
        let mut byte = [0u8; 1];
        assert!(stream.read(&mut byte).is_err());

        let mut conn = Vec::new();
        write_record(
            &mut conn,
            &Record::new(super::RECORD_TYPE_ALERT, VERSION_12, vec![1]),
        )
        .unwrap();
        let mut stream = FakeTls::new(Cursor::new(conn));
        assert!(stream.read(&mut byte).is_err());
    }

    #[test]
    fn fake_tls_write_sends_change_cipher_spec_first() {
        let mut stream = FakeTls::new(Cursor::new(Vec::new()));
        stream.write_all(b"payload").unwrap();
        let wire = stream.into_inner().into_inner();
        let mut cursor = Cursor::new(wire);
        let first = read_record(&mut cursor).unwrap();
        assert_eq!(first.record_type, RECORD_TYPE_CHANGE_CIPHER_SPEC);
        assert_eq!(first.data, vec![0x01]);
        let second = read_record(&mut cursor).unwrap();
        assert_eq!(second.record_type, RECORD_TYPE_APPLICATION);
        assert_eq!(second.data, b"payload");
    }

    #[test]
    fn client_handshake_reports_missing_response() {
        // The client writes its hello and then fails to read a ServerHello.
        let mut conn = Cursor::new(Vec::new());
        assert!(client_handshake(&mut conn, SECRET, "example.com").is_err());
    }

    #[test]
    fn fake_tls_handshake_runs_over_a_stream() {
        // Build a valid client hello and verify it, mirroring the server half
        // of the handshake, then confirm the response verifies on the client.
        let mut to_server = Vec::new();
        let client_random = write_client_hello(&mut to_server, 42, "example.com", SECRET).unwrap();
        let verified = verify_client_hello(&to_server, SECRET).unwrap();
        assert_eq!(verified, client_random);

        let mut to_client = Vec::new();
        write_server_hello(&mut to_client, verified, SECRET).unwrap();
        read_server_hello(&mut Cursor::new(to_client), verified, SECRET).unwrap();
    }
}
