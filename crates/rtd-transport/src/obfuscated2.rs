//! The obfuscated2 transport used by MTProxy.
//!
//! A 64-byte random init forms the base of an AES-CTR key pair. Both sides
//! derive the same keys and the first 56 header bytes are sent in plaintext
//! while the remaining 8 are XOR-encrypted.
//!
//! See <https://core.telegram.org/mtproto/mtproto-transports#transport-obfuscation>.

use crate::error::{Error, Result};
use aes::Aes256;
use aes::cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

/// An AES-256 CTR stream in big-endian counter mode, matching Go's
/// `cipher.NewCTR`.
type Aes256Ctr = Ctr128BE<Aes256>;

/// The number of random bytes in an obfuscated2 init/header.
const INIT_LEN: usize = 64;

/// Maximum attempts to produce a random init that passes the filter.
const MAX_INIT_ATTEMPTS: usize = 64;

/// Transport metadata recovered from an obfuscated2 header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    /// Protocol tag requested by the client (`0xef..`, `0xee..`, `0xdd..`).
    pub protocol: [u8; 4],
    /// Data-center label from the header.
    pub dc: u16,
}

/// The encrypt and decrypt AES-CTR streams of one obfuscated2 endpoint.
struct Keys {
    encrypt: Aes256Ctr,
    decrypt: Aes256Ctr,
}

impl Keys {
    /// Derives both streams from `init` and an optional secret.
    fn create(init: &[u8], secret: &[u8]) -> Result<Self> {
        let encrypt_key = &init[8..40];
        let encrypt_iv = &init[40..56];

        let mut reversed = [0u8; 48];
        reversed.copy_from_slice(&init[8..56]);
        reversed.reverse();
        let decrypt_key = &reversed[..32];
        let decrypt_iv = &reversed[32..48];

        let (encrypt_key, decrypt_key) = if secret.is_empty() {
            (encrypt_key.to_vec(), decrypt_key.to_vec())
        } else {
            if secret.len() < 16 {
                return Err(Error::InvalidSecretLength(secret.len()));
            }
            let secret = &secret[..16];
            (
                sha256_concat(encrypt_key, secret).to_vec(),
                sha256_concat(decrypt_key, secret).to_vec(),
            )
        };

        let encrypt = Aes256Ctr::new_from_slices(&encrypt_key, encrypt_iv)
            .map_err(|_| Error::InvalidSecretLength(encrypt_key.len()))?;
        let decrypt = Aes256Ctr::new_from_slices(&decrypt_key, decrypt_iv)
            .map_err(|_| Error::InvalidSecretLength(decrypt_key.len()))?;

        Ok(Self { encrypt, decrypt })
    }
}

/// Computes `SHA256(a || b)`.
fn sha256_concat(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(a);
    hasher.update(b);
    hasher.finalize().into()
}

/// Returns `true` if `init` does not begin with a filtered byte sequence.
fn is_valid_init(init: &[u8; INIT_LEN]) -> bool {
    if init[0] == 0xef {
        return false;
    }
    let first = u32::from_le_bytes([init[0], init[1], init[2], init[3]]);
    if matches!(
        first,
        0x4441_4548 // "HEAD"
            | 0x5453_4f50 // "POST"
            | 0x2054_4547 // "GET "
            | 0x4954_504f // "OPTI"
            | 0x0201_0316
            | 0xdddd_dddd
            | 0xeeee_eeee
    ) {
        return false;
    }
    let second = u32::from_le_bytes([init[4], init[5], init[6], init[7]]);
    second != 0
}

/// Generates a random init accepted by the filter.
fn generate_init() -> Result<[u8; INIT_LEN]> {
    for _ in 0..MAX_INIT_ATTEMPTS {
        let mut init = [0u8; INIT_LEN];
        getrandom::fill(&mut init).map_err(|err| Error::Io(std::io::Error::other(err)))?;
        if is_valid_init(&init) {
            return Ok(init);
        }
    }
    Err(Error::InvalidInit)
}

/// Derives the key streams and the plaintext/encrypted header from `init`.
fn generate_keys(
    init: &[u8; INIT_LEN],
    protocol: [u8; 4],
    dc: u16,
    secret: &[u8],
) -> Result<([u8; INIT_LEN], Keys)> {
    let mut keys = Keys::create(init, secret)?;

    let mut plain = *init;
    plain[56..60].copy_from_slice(&protocol);
    plain[60..62].copy_from_slice(&dc.to_le_bytes());

    let mut encrypted = plain;
    keys.encrypt.apply_keystream(&mut encrypted);

    let mut header = plain;
    header[56..64].copy_from_slice(&encrypted[56..64]);

    Ok((header, keys))
}

/// An obfuscated2 stream wrapping an underlying connection.
pub struct Obfuscated2<S> {
    conn: S,
    encrypt: Aes256Ctr,
    decrypt: Aes256Ctr,
}

impl<S: Read + Write> Obfuscated2<S> {
    /// Performs the client handshake and returns the wrapped stream.
    ///
    /// # Errors
    ///
    /// Returns an error if a valid init cannot be generated, the secret is too
    /// short, or writing the header fails.
    pub fn handshake(mut conn: S, protocol: [u8; 4], dc: u16, secret: &[u8]) -> Result<Self> {
        let init = generate_init()?;
        let (header, keys) = generate_keys(&init, protocol, dc, secret)?;
        conn.write_all(&header)?;
        Ok(Self {
            conn,
            encrypt: keys.encrypt,
            decrypt: keys.decrypt,
        })
    }

    /// Performs the server handshake and returns the wrapped stream together
    /// with the metadata recovered from the header.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the header fails or the secret is too short.
    pub fn accept(mut conn: S, secret: &[u8]) -> Result<(Self, Metadata)> {
        let mut init = [0u8; INIT_LEN];
        conn.read_exact(&mut init)?;

        let mut keys = Keys::create(&init, secret)?;
        // Swap so the server's outgoing stream matches the client's decrypt
        // stream and vice versa.
        core::mem::swap(&mut keys.encrypt, &mut keys.decrypt);

        let mut decrypted = init;
        keys.decrypt.apply_keystream(&mut decrypted);

        let mut protocol = [0u8; 4];
        protocol.copy_from_slice(&decrypted[56..60]);
        let dc = u16::from_le_bytes([decrypted[60], decrypted[61]]);

        Ok((
            Self {
                conn,
                encrypt: keys.encrypt,
                decrypt: keys.decrypt,
            },
            Metadata { protocol, dc },
        ))
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

impl<S: Read + Write> Read for Obfuscated2<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.conn.read(buf)?;
        if n > 0 {
            self.decrypt.apply_keystream(&mut buf[..n]);
        }
        Ok(n)
    }
}

impl<S: Read + Write> Write for Obfuscated2<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut encrypted = buf.to_vec();
        self.encrypt.apply_keystream(&mut encrypted);
        self.conn.write(&encrypted)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.conn.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::{Keys, generate_keys, is_valid_init, sha256_concat};
    use crate::obfuscated2::Obfuscated2;
    use aes::cipher::StreamCipher;
    use std::io::{Cursor, Read, Write};

    fn hex(value: &str) -> Vec<u8> {
        (0..value.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn init_filter() {
        let mut init = [0x11u8; 64];
        assert!(is_valid_init(&init));
        init[0] = 0xef;
        assert!(!is_valid_init(&init));
        init[0] = b'H';
        init[1] = b'E';
        init[2] = b'A';
        init[3] = b'D';
        assert!(!is_valid_init(&init));
        let mut init = [0x11u8; 64];
        init[4..8].copy_from_slice(&[0, 0, 0, 0]);
        assert!(!is_valid_init(&init));
    }

    #[test]
    fn golden_header_with_all_a() {
        // Mirrors the reference Test_generateKeys: init is 64 bytes of 'a',
        // secret is the 16-byte value 0xaa repeated, protocol dd, dc 2.
        let init = [b'a'; 64];
        let secret = hex(&"a".repeat(32));
        let (header, _keys) = generate_keys(&init, [0xdd; 4], 2, &secret).unwrap();
        let mut expected = vec![b'a'; 56];
        expected.extend_from_slice(&[171, 65, 98, 66, 79, 102, 253, 220]);
        assert_eq!(header.to_vec(), expected);
    }

    #[test]
    fn golden_encrypt_streams() {
        // Mirrors the reference TestEncrypt.
        let init: [u8; 64] = [
            245, 118, 143, 80, 183, 49, 38, 10, 70, 190, 16, 39, 194, 238, 170, 57, 53, 6, 36, 240,
            182, 218, 89, 235, 165, 108, 129, 254, 69, 16, 194, 224, 182, 29, 61, 211, 35, 238, 2,
            56, 134, 51, 227, 131, 122, 12, 28, 36, 250, 111, 41, 204, 215, 36, 190, 111, 65, 111,
            247, 176, 38, 246, 204, 230,
        ];
        let secret = hex("8a96ef6e42a18c21837580cd1c91c5a8");
        let (header, mut keys) = generate_keys(&init, [0xdd; 4], 2, &secret).unwrap();

        let mut expected = init;
        expected[56..64].copy_from_slice(&[190, 162, 221, 225, 109, 197, 157, 210]);
        assert_eq!(header.to_vec(), expected.to_vec());

        let mut encrypted = *b"abcd";
        keys.encrypt.apply_keystream(&mut encrypted);
        assert_eq!(encrypted, [202, 122, 130, 38]);

        let mut decrypted = *b"abcd";
        keys.decrypt.apply_keystream(&mut decrypted);
        assert_eq!(decrypted, [143, 113, 25, 130]);
    }

    #[test]
    fn decrypt_init_is_reversed_tail() {
        let init: [u8; 64] = core::array::from_fn(|i| i as u8);
        let mut reversed = [0u8; 48];
        reversed.copy_from_slice(&init[8..56]);
        reversed.reverse();
        assert_eq!(reversed[0], 55);
        assert_eq!(reversed[47], 8);
    }

    #[test]
    fn stream_round_trip_and_metadata() {
        let protocol = [0xdd, 0xdd, 0xdd, 0xdd];
        let secret = b"0123456789abcdef";

        let mut client =
            Obfuscated2::handshake(Cursor::new(Vec::new()), protocol, 7, secret).unwrap();
        client.write_all(b"ping from client").unwrap();
        let wire = client.into_inner().into_inner();

        let (mut server, metadata) = Obfuscated2::accept(Cursor::new(wire), secret).unwrap();
        assert_eq!(metadata.protocol, protocol);
        assert_eq!(metadata.dc, 7);

        let mut received = vec![0u8; "ping from client".len()];
        server.read_exact(&mut received).unwrap();
        assert_eq!(&received, b"ping from client");
    }

    #[test]
    fn both_directions_with_shared_init() {
        // Both endpoints derive from the same init so both directions can be
        // exercised without a network round trip.
        let init = [0x5au8; 64];
        let protocol = [0xee, 0xee, 0xee, 0xee];
        let secret = b"abcdefghijklmnop";
        let (header, mut client_keys) = generate_keys(&init, protocol, 3, secret).unwrap();

        let mut server_keys = Keys::create(&init, secret).unwrap();
        core::mem::swap(&mut server_keys.encrypt, &mut server_keys.decrypt);
        // The server decrypts the full header, advancing its incoming stream by
        // 64 bytes to match the client's outgoing stream.
        let mut header_copy = header;
        server_keys.decrypt.apply_keystream(&mut header_copy);

        let mut to_server = b"client->server".to_vec();
        client_keys.encrypt.apply_keystream(&mut to_server);
        server_keys.decrypt.apply_keystream(&mut to_server);
        assert_eq!(&to_server, b"client->server");

        let mut to_client = b"server->client".to_vec();
        server_keys.encrypt.apply_keystream(&mut to_client);
        client_keys.decrypt.apply_keystream(&mut to_client);
        assert_eq!(&to_client, b"server->client");
    }

    #[test]
    fn short_secret_is_rejected() {
        let init = [0x11u8; 64];
        assert!(Keys::create(&init, &[1, 2, 3]).is_err());
    }

    #[test]
    fn sha256_concat_matches_manual() {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"foo");
        hasher.update(b"bar");
        let expected: [u8; 32] = hasher.finalize().into();
        assert_eq!(sha256_concat(b"foo", b"bar"), expected);
    }

    /// A reader that yields one byte at a time, to exercise partial reads.
    struct OneByte {
        data: Vec<u8>,
        pos: usize,
    }

    impl Read for OneByte {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.data.len() || buf.is_empty() {
                return Ok(0);
            }
            buf[0] = self.data[self.pos];
            self.pos += 1;
            Ok(1)
        }
    }

    impl Write for OneByte {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.data.extend_from_slice(buf);
            self.pos = self.data.len();
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn read_applies_keystream_only_to_bytes_read() {
        let protocol = [0xdd; 4];
        let secret = b"secretsecretsecr";

        let mut client =
            Obfuscated2::handshake(Cursor::new(Vec::new()), protocol, 2, secret).unwrap();

        let mut payload = vec![0u8; 5000];
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }
        client.write_all(&payload).unwrap();
        let wire = client.into_inner().into_inner();

        // The server reads the header and payload one byte at a time.
        let mut server = Obfuscated2::accept(OneByte { data: wire, pos: 0 }, secret)
            .unwrap()
            .0;

        let mut decoded = vec![0u8; payload.len()];
        server.read_exact(&mut decoded).unwrap();
        assert_eq!(decoded, payload);
    }
}
