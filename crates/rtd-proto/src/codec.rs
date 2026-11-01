//! MTProto transport codecs.
//!
//! Each codec wraps an MTProto payload into a transport frame. The built-in
//! framings are abridged, intermediate, padded-intermediate and full. A single
//! trailing word in a frame is interpreted as a transport protocol error code.
//!
//! See <https://core.telegram.org/mtproto/mtproto-transports>.

use core::fmt;
use std::io::{Read, Write};

use rtd_util::random::{Random, SystemRandom};

/// Maximum transport frame payload accepted by the built-in codecs (16 MiB).
pub const MAX_MESSAGE_SIZE: usize = 1 << 24;

/// Chunk size used when reading a payload from an untrusted length prefix.
const INBOUND_READ_CHUNK_SIZE: usize = 32 << 10;

/// Abridged protocol header byte.
pub const ABRIDGED_HEADER: u8 = 0xef;
/// Intermediate protocol header.
pub const INTERMEDIATE_HEADER: [u8; 4] = [0xee, 0xee, 0xee, 0xee];
/// Padded-intermediate protocol header.
pub const PADDED_INTERMEDIATE_HEADER: [u8; 4] = [0xdd, 0xdd, 0xdd, 0xdd];

/// Transport protocol error code: the auth-key id is unknown to the DC.
pub const CODE_AUTH_KEY_NOT_FOUND: i32 = 404;
/// Transport protocol error code: the DC is wrong.
pub const CODE_WRONG_DC: i32 = 444;
/// Transport protocol error code: transport flood or limit reached.
pub const CODE_TRANSPORT_FLOOD: i32 = 429;

/// A transport-level protocol error carried in a single-word frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolError {
    /// The (positive) error code reported by the peer.
    pub code: i32,
}

impl ProtocolError {
    /// Creates a protocol error with the given code.
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self { code }
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            CODE_AUTH_KEY_NOT_FOUND => f.write_str("auth key not found"),
            CODE_TRANSPORT_FLOOD => f.write_str("transport flood"),
            CODE_WRONG_DC => f.write_str("wrong DC"),
            code => write!(f, "protocol error {code}"),
        }
    }
}

impl core::error::Error for ProtocolError {}

/// An error produced by a transport codec.
#[derive(Debug)]
#[non_exhaustive]
pub enum CodecError {
    /// An underlying I/O error.
    Io(std::io::Error),
    /// A frame declared an invalid or unsupported length.
    InvalidLength(usize),
    /// The peer reported a transport protocol error.
    Protocol(ProtocolError),
    /// A protocol header did not match the expected value.
    HeaderMismatch,
    /// A full-framing sequence number did not match the expected value.
    SeqNoMismatch,
    /// A full-framing CRC checksum did not match.
    CrcMismatch,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::InvalidLength(len) => write!(f, "invalid message length {len}"),
            Self::Protocol(err) => write!(f, "{err}"),
            Self::HeaderMismatch => f.write_str("protocol header mismatch"),
            Self::SeqNoMismatch => f.write_str("seq_no mismatch"),
            Self::CrcMismatch => f.write_str("crc mismatch"),
        }
    }
}

impl core::error::Error for CodecError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Protocol(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CodecError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// An MTProto transport codec.
pub trait Codec {
    /// Writes the protocol header, if the framing has one.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying writer fails.
    fn write_header(&mut self, w: &mut dyn Write) -> Result<(), CodecError>;

    /// Reads and validates the protocol header, if the framing has one.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying reader fails or the header is
    /// mismatched.
    fn read_header(&mut self, r: &mut dyn Read) -> Result<(), CodecError>;

    /// Writes a framed message with the given payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is not valid for this framing or the
    /// underlying writer fails.
    fn write_message(&mut self, w: &mut dyn Write, payload: &[u8]) -> Result<(), CodecError>;

    /// Reads a framed message and returns its payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame is malformed, the peer reported a protocol
    /// error, or the underlying reader fails.
    fn read_message(&mut self, r: &mut dyn Read) -> Result<Vec<u8>, CodecError>;

    /// Returns the protocol tag used for obfuscation, if this codec has one.
    #[must_use]
    fn obfuscated_tag(&self) -> Option<[u8; 4]>;
}

/// Validates an outgoing or incoming frame payload length.
fn check_length(length: usize) -> Result<(), CodecError> {
    if length == 0 || length > MAX_MESSAGE_SIZE {
        Err(CodecError::InvalidLength(length))
    } else {
        Ok(())
    }
}

/// Reads exactly `n` bytes, growing the destination in bounded increments.
///
/// The declared length is never used to pre-allocate the whole frame, so a peer
/// sending only a large length prefix cannot force a large allocation.
fn read_payload(r: &mut dyn Read, n: usize) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    let mut remaining = n;
    while remaining > 0 {
        let chunk = remaining.min(INBOUND_READ_CHUNK_SIZE);
        let start = out.len();
        out.resize(start + chunk, 0);
        r.read_exact(&mut out[start..start + chunk])?;
        remaining -= chunk;
    }
    Ok(out)
}

/// Interprets a four-byte payload as a transport protocol error.
fn check_protocol_error(payload: Vec<u8>) -> Result<Vec<u8>, CodecError> {
    if payload.len() == 4 {
        let bytes = [payload[0], payload[1], payload[2], payload[3]];
        let code = i32::from_le_bytes(bytes);
        return Err(CodecError::Protocol(ProtocolError {
            code: code.wrapping_neg(),
        }));
    }
    Ok(payload)
}

/// Builds a four-byte little-endian frame length.
fn length_word(len: usize) -> [u8; 4] {
    (len as u32).to_le_bytes()
}

/// Abridged MTProto transport.
///
/// The payload length divided by four is encoded as a single byte, or as
/// `0x7f` followed by three little-endian bytes when it is at least `0x7f`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Abridged;

impl Codec for Abridged {
    fn write_header(&mut self, w: &mut dyn Write) -> Result<(), CodecError> {
        w.write_all(&[ABRIDGED_HEADER])?;
        Ok(())
    }

    fn read_header(&mut self, r: &mut dyn Read) -> Result<(), CodecError> {
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte)?;
        if byte[0] != ABRIDGED_HEADER {
            return Err(CodecError::HeaderMismatch);
        }
        Ok(())
    }

    fn write_message(&mut self, w: &mut dyn Write, payload: &[u8]) -> Result<(), CodecError> {
        check_length(payload.len())?;
        if !payload.len().is_multiple_of(4) {
            return Err(CodecError::InvalidLength(payload.len()));
        }
        let words = payload.len() / 4;
        if words < 0x7f {
            w.write_all(&[words as u8])?;
        } else {
            let bytes = length_word(words);
            w.write_all(&[0x7f, bytes[0], bytes[1], bytes[2]])?;
        }
        w.write_all(payload)?;
        Ok(())
    }

    fn read_message(&mut self, r: &mut dyn Read) -> Result<Vec<u8>, CodecError> {
        let mut first = [0u8; 1];
        r.read_exact(&mut first)?;
        let words = u32::from(first[0] & 0x7f);
        if words == 0 {
            return Err(CodecError::InvalidLength(0));
        }
        let words = if words < 0x7f {
            words
        } else {
            let mut extended = [0u8; 3];
            r.read_exact(&mut extended)?;
            let words = u32::from(extended[0])
                | (u32::from(extended[1]) << 8)
                | (u32::from(extended[2]) << 16);
            if words < 0x7f {
                return Err(CodecError::InvalidLength((words as usize) * 4));
            }
            words
        };
        let n = (words as usize) * 4;
        check_length(n)?;
        let payload = read_payload(r, n)?;
        check_protocol_error(payload)
    }

    fn obfuscated_tag(&self) -> Option<[u8; 4]> {
        Some([ABRIDGED_HEADER; 4])
    }
}

/// Intermediate MTProto transport.
///
/// The payload length is encoded as a four-byte little-endian integer.
#[derive(Debug, Default, Clone, Copy)]
pub struct Intermediate;

impl Codec for Intermediate {
    fn write_header(&mut self, w: &mut dyn Write) -> Result<(), CodecError> {
        w.write_all(&INTERMEDIATE_HEADER)?;
        Ok(())
    }

    fn read_header(&mut self, r: &mut dyn Read) -> Result<(), CodecError> {
        let mut header = [0u8; 4];
        r.read_exact(&mut header)?;
        if header != INTERMEDIATE_HEADER {
            return Err(CodecError::HeaderMismatch);
        }
        Ok(())
    }

    fn write_message(&mut self, w: &mut dyn Write, payload: &[u8]) -> Result<(), CodecError> {
        check_length(payload.len())?;
        if !payload.len().is_multiple_of(4) {
            return Err(CodecError::InvalidLength(payload.len()));
        }
        w.write_all(&length_word(payload.len()))?;
        w.write_all(payload)?;
        Ok(())
    }

    fn read_message(&mut self, r: &mut dyn Read) -> Result<Vec<u8>, CodecError> {
        let mut len = [0u8; 4];
        r.read_exact(&mut len)?;
        let n = u32::from_le_bytes(len) as usize;
        check_length(n)?;
        let payload = read_payload(r, n)?;
        check_protocol_error(payload)
    }

    fn obfuscated_tag(&self) -> Option<[u8; 4]> {
        Some(INTERMEDIATE_HEADER)
    }
}

/// Padded-intermediate MTProto transport.
///
/// Like [`Intermediate`], but the payload is padded to a multiple of four bytes
/// with a number of random bytes equal to the last payload byte modulo four.
///
/// The padding bytes come from an injectable [`Random`] source so the core stays
/// deterministic; the default is the operating-system CSPRNG.
#[derive(Debug, Clone, Copy)]
pub struct PaddedIntermediate<R = SystemRandom> {
    random: R,
}

impl PaddedIntermediate<SystemRandom> {
    /// Creates a padded-intermediate codec that pads frames with the operating
    /// system CSPRNG.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            random: SystemRandom,
        }
    }
}

impl Default for PaddedIntermediate<SystemRandom> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> PaddedIntermediate<R> {
    /// Creates a padded-intermediate codec that pads frames using `random`.
    pub const fn with_random(random: R) -> Self {
        Self { random }
    }
}

impl<R: Random> Codec for PaddedIntermediate<R> {
    fn write_header(&mut self, w: &mut dyn Write) -> Result<(), CodecError> {
        w.write_all(&PADDED_INTERMEDIATE_HEADER)?;
        Ok(())
    }

    fn read_header(&mut self, r: &mut dyn Read) -> Result<(), CodecError> {
        let mut header = [0u8; 4];
        r.read_exact(&mut header)?;
        if header != PADDED_INTERMEDIATE_HEADER {
            return Err(CodecError::HeaderMismatch);
        }
        Ok(())
    }

    fn write_message(&mut self, w: &mut dyn Write, payload: &[u8]) -> Result<(), CodecError> {
        check_length(payload.len())?;
        if !payload.len().is_multiple_of(4) {
            return Err(CodecError::InvalidLength(payload.len()));
        }
        let padding_len = usize::from(payload[payload.len() - 1] % 4);
        let mut padding = [0u8; 3];
        self.random.fill(&mut padding);
        let frame_len = payload.len() + padding_len;
        w.write_all(&length_word(frame_len))?;
        w.write_all(payload)?;
        w.write_all(&padding[..padding_len])?;
        Ok(())
    }

    fn read_message(&mut self, r: &mut dyn Read) -> Result<Vec<u8>, CodecError> {
        let mut len = [0u8; 4];
        r.read_exact(&mut len)?;
        let frame_len = u32::from_le_bytes(len) as usize;
        check_length(frame_len)?;
        let mut payload = read_payload(r, frame_len)?;
        let padding_len = frame_len % 4;
        payload.truncate(payload.len() - padding_len);
        check_protocol_error(payload)
    }

    fn obfuscated_tag(&self) -> Option<[u8; 4]> {
        Some(PADDED_INTERMEDIATE_HEADER)
    }
}

/// Full MTProto transport.
///
/// Each frame is `length | seq_no | payload | crc`, where `length` counts all
/// four words and `crc` is the CRC32-IEEE of everything before it. Sequence
/// numbers start at zero and increment per message in each direction.
#[derive(Debug, Default, Clone, Copy)]
pub struct Full {
    write_seq_no: i32,
    read_seq_no: i32,
}

impl Full {
    /// Creates a full-framing codec with both sequence counters at zero.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            write_seq_no: 0,
            read_seq_no: 0,
        }
    }
}

impl Codec for Full {
    fn write_header(&mut self, _w: &mut dyn Write) -> Result<(), CodecError> {
        Ok(())
    }

    fn read_header(&mut self, _r: &mut dyn Read) -> Result<(), CodecError> {
        Ok(())
    }

    fn write_message(&mut self, w: &mut dyn Write, payload: &[u8]) -> Result<(), CodecError> {
        let seq_no = self.write_seq_no;
        self.write_seq_no = self.write_seq_no.wrapping_add(1);
        check_length(payload.len())?;
        let frame_len = 4 + 4 + payload.len() + 4;

        let mut frame = Vec::with_capacity(frame_len);
        frame.extend_from_slice(&length_word(frame_len));
        frame.extend_from_slice(&seq_no.to_le_bytes());
        frame.extend_from_slice(payload);
        let crc = crc32fast::hash(&frame);
        frame.extend_from_slice(&crc.to_le_bytes());

        w.write_all(&frame)?;
        Ok(())
    }

    fn read_message(&mut self, r: &mut dyn Read) -> Result<Vec<u8>, CodecError> {
        let expected_seq_no = self.read_seq_no;
        self.read_seq_no = self.read_seq_no.wrapping_add(1);

        let mut len = [0u8; 4];
        r.read_exact(&mut len)?;
        let frame_len = u32::from_le_bytes(len) as usize;
        check_length(frame_len)?;
        if frame_len < 3 * 4 {
            return Err(CodecError::InvalidLength(frame_len));
        }

        let body = read_payload(r, frame_len - 4)?;
        let mut frame = Vec::with_capacity(frame_len);
        frame.extend_from_slice(&len);
        frame.extend_from_slice(&body);

        let mut seq_bytes = [0u8; 4];
        seq_bytes.copy_from_slice(&frame[4..8]);
        if i32::from_le_bytes(seq_bytes) != expected_seq_no {
            return Err(CodecError::SeqNoMismatch);
        }

        let crc_offset = frame_len - 4;
        let mut crc_bytes = [0u8; 4];
        crc_bytes.copy_from_slice(&frame[crc_offset..frame_len]);
        let expected_crc = u32::from_le_bytes(crc_bytes);
        let actual_crc = crc32fast::hash(&frame[..crc_offset]);
        if actual_crc != expected_crc {
            return Err(CodecError::CrcMismatch);
        }

        let payload = frame[8..crc_offset].to_vec();
        check_protocol_error(payload)
    }

    fn obfuscated_tag(&self) -> Option<[u8; 4]> {
        None
    }
}

/// A codec wrapper that suppresses protocol-header reads and writes.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoHeader<C> {
    inner: C,
}

impl<C> NoHeader<C> {
    /// Wraps `inner`.
    #[must_use]
    pub const fn new(inner: C) -> Self {
        Self { inner }
    }

    /// Returns a reference to the wrapped codec.
    #[must_use]
    pub const fn inner(&self) -> &C {
        &self.inner
    }

    /// Consumes the wrapper and returns the wrapped codec.
    #[must_use]
    pub fn into_inner(self) -> C {
        self.inner
    }
}

impl<C: Codec> Codec for NoHeader<C> {
    fn write_header(&mut self, _w: &mut dyn Write) -> Result<(), CodecError> {
        Ok(())
    }

    fn read_header(&mut self, _r: &mut dyn Read) -> Result<(), CodecError> {
        Ok(())
    }

    fn write_message(&mut self, w: &mut dyn Write, payload: &[u8]) -> Result<(), CodecError> {
        self.inner.write_message(w, payload)
    }

    fn read_message(&mut self, r: &mut dyn Read) -> Result<Vec<u8>, CodecError> {
        self.inner.read_message(r)
    }

    fn obfuscated_tag(&self) -> Option<[u8; 4]> {
        self.inner.obfuscated_tag()
    }
}

/// A transport framing identified from a connection's first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecKind {
    /// Abridged framing.
    Abridged,
    /// Intermediate framing.
    Intermediate,
    /// Padded-intermediate framing.
    PaddedIntermediate,
    /// Full framing.
    Full,
}

impl CodecKind {
    /// Detects the framing from the first bytes of a connection.
    ///
    /// A leading `0xef` selects abridged. Otherwise the first four bytes are
    /// compared against the intermediate and padded-intermediate headers; any
    /// other value selects full framing.
    #[must_use]
    pub fn detect(header: &[u8]) -> Self {
        if header.first() == Some(&ABRIDGED_HEADER) {
            return Self::Abridged;
        }
        if header.len() >= 4 {
            match &header[..4] {
                [0xee, 0xee, 0xee, 0xee] => Self::Intermediate,
                [0xdd, 0xdd, 0xdd, 0xdd] => Self::PaddedIntermediate,
                _ => Self::Full,
            }
        } else {
            Self::Full
        }
    }

    /// Builds a boxed codec for this framing.
    #[must_use]
    pub fn build(self) -> Box<dyn Codec> {
        match self {
            Self::Abridged => Box::new(Abridged),
            Self::Intermediate => Box::new(Intermediate),
            Self::PaddedIntermediate => Box::new(PaddedIntermediate::new()),
            Self::Full => Box::new(Full::new()),
        }
    }

    /// Returns the protocol tag used for obfuscation, if this framing has one.
    #[must_use]
    pub fn obfuscated_tag(self) -> Option<[u8; 4]> {
        match self {
            Self::Abridged => Some([ABRIDGED_HEADER; 4]),
            Self::Intermediate => Some(INTERMEDIATE_HEADER),
            Self::PaddedIntermediate => Some(PADDED_INTERMEDIATE_HEADER),
            Self::Full => None,
        }
    }
}

/// Detects the transport framing from the first bytes of a connection.
#[must_use]
pub fn detect_codec(header: &[u8]) -> CodecKind {
    CodecKind::detect(header)
}

#[cfg(test)]
mod tests {
    use rtd_util::random::DeterministicRandom;

    use super::{
        ABRIDGED_HEADER, Abridged, Codec, CodecError, CodecKind, Full, INTERMEDIATE_HEADER,
        Intermediate, MAX_MESSAGE_SIZE, NoHeader, PADDED_INTERMEDIATE_HEADER, PaddedIntermediate,
        ProtocolError, detect_codec,
    };
    use std::io::{Cursor, Read};

    fn round_trip<C: Codec + ?Sized>(codec: &mut C, payload: &[u8]) {
        let mut buffer = Vec::new();
        codec.write_message(&mut buffer, payload).unwrap();
        let mut cursor = Cursor::new(buffer);
        let decoded = codec.read_message(&mut cursor).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn abridged_short_golden() {
        let mut buffer = Vec::new();
        Abridged.write_message(&mut buffer, &[1, 2, 3, 4]).unwrap();
        assert_eq!(buffer, vec![0x01, 1, 2, 3, 4]);
    }

    #[test]
    fn abridged_extended_golden() {
        let payload = vec![7u8; 127 * 4];
        let mut buffer = Vec::new();
        Abridged.write_message(&mut buffer, &payload).unwrap();
        assert_eq!(&buffer[..4], &[0x7f, 0x7f, 0x00, 0x00]);
        assert_eq!(buffer.len(), 4 + payload.len());
    }

    #[test]
    fn abridged_round_trip() {
        let mut codec = Abridged;
        round_trip(&mut codec, &[1, 2, 3, 4, 5, 6, 7, 8]);
        round_trip(&mut codec, &vec![9u8; 1024]);
    }

    #[test]
    fn abridged_rejects_misaligned_payload() {
        let mut buffer = Vec::new();
        assert!(matches!(
            Abridged.write_message(&mut buffer, &[1, 2, 3]),
            Err(CodecError::InvalidLength(3))
        ));
    }

    #[test]
    fn abridged_rejects_zero_length() {
        let mut buffer = Vec::new();
        assert!(matches!(
            Abridged.write_message(&mut buffer, &[]),
            Err(CodecError::InvalidLength(0))
        ));
    }

    #[test]
    fn abridged_header_round_trip() {
        let mut buffer = Vec::new();
        Abridged.write_header(&mut buffer).unwrap();
        assert_eq!(buffer, vec![ABRIDGED_HEADER]);
        Abridged.read_header(&mut Cursor::new(buffer)).unwrap();
        assert!(matches!(
            Abridged.read_header(&mut Cursor::new(vec![0x00])),
            Err(CodecError::HeaderMismatch)
        ));
    }

    #[test]
    fn intermediate_golden() {
        let mut buffer = Vec::new();
        Intermediate
            .write_message(&mut buffer, &[9, 9, 9, 9])
            .unwrap();
        assert_eq!(buffer, vec![4, 0, 0, 0, 9, 9, 9, 9]);
    }

    #[test]
    fn intermediate_round_trip() {
        let mut codec = Intermediate;
        round_trip(&mut codec, &[1, 2, 3, 4, 5, 6, 7, 8]);
        round_trip(&mut codec, &vec![3u8; 4096]);
    }

    #[test]
    fn intermediate_header_round_trip() {
        let mut buffer = Vec::new();
        Intermediate.write_header(&mut buffer).unwrap();
        assert_eq!(buffer, INTERMEDIATE_HEADER.to_vec());
        Intermediate.read_header(&mut Cursor::new(buffer)).unwrap();
    }

    #[test]
    fn padded_intermediate_round_trip() {
        let mut codec = PaddedIntermediate::new();
        // Last byte four makes the padding length zero.
        round_trip(&mut codec, &[1, 2, 3, 4, 5, 6, 7, 4]);
        // Last byte five makes the padding length one.
        round_trip(&mut codec, &[1, 2, 3, 4, 5, 6, 7, 5]);
        // Last byte seven makes the padding length three.
        round_trip(&mut codec, &[1, 2, 3, 4, 5, 6, 7, 7]);
    }

    #[test]
    fn padded_intermediate_header_round_trip() {
        let mut buffer = Vec::new();
        PaddedIntermediate::new().write_header(&mut buffer).unwrap();
        assert_eq!(buffer, PADDED_INTERMEDIATE_HEADER.to_vec());
        PaddedIntermediate::new()
            .read_header(&mut Cursor::new(buffer))
            .unwrap();
    }

    #[test]
    fn padded_intermediate_frame_length_tracks_padding() {
        let mut buffer = Vec::new();
        PaddedIntermediate::new()
            .write_message(&mut buffer, &[1, 2, 3, 4, 5, 6, 7, 5])
            .unwrap();
        let frame_len = u32::from_le_bytes(buffer[..4].try_into().unwrap());
        assert_eq!(frame_len, 9);
    }

    #[test]
    fn padded_intermediate_is_deterministic_with_injected_randomness() {
        fn frame(seed: u64) -> Vec<u8> {
            let mut codec = PaddedIntermediate::with_random(DeterministicRandom::new(seed));
            let mut buffer = Vec::new();
            codec
                .write_message(&mut buffer, &[1, 2, 3, 4, 5, 6, 7, 7])
                .unwrap();
            buffer
        }
        // Same seed -> byte-identical frames; different seed -> different padding.
        assert_eq!(frame(7), frame(7));
        assert_ne!(frame(7), frame(8));
    }

    /// Builds a full frame with the given sequence number and a valid CRC.
    fn full_frame(seq_no: i32, payload: &[u8]) -> Vec<u8> {
        let frame_len = 4 + 4 + payload.len() + 4;
        let mut frame = Vec::with_capacity(frame_len);
        frame.extend_from_slice(&(frame_len as u32).to_le_bytes());
        frame.extend_from_slice(&seq_no.to_le_bytes());
        frame.extend_from_slice(payload);
        let crc = crc32fast::hash(&frame);
        frame.extend_from_slice(&crc.to_le_bytes());
        frame
    }

    #[test]
    fn full_golden() {
        let payload = [1u8, 2, 3, 4];
        let mut buffer = Vec::new();
        Full::new().write_message(&mut buffer, &payload).unwrap();
        // Frame: length = 16, seq = 0, payload, CRC32-IEEE.
        let expected = vec![
            16, 0, 0, 0, // length
            0, 0, 0, 0, // seq_no
            1, 2, 3, 4, // payload
            0x42, 0x0f, 0xb5, 0xe0, // crc32
        ];
        assert_eq!(buffer, expected);
    }

    #[test]
    fn full_round_trip_advances_sequence() {
        let mut writer = Full::new();
        let mut first = Vec::new();
        let mut second = Vec::new();
        writer
            .write_message(&mut first, &[1, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
        writer
            .write_message(&mut second, &[9, 10, 11, 12, 13, 14, 15, 16])
            .unwrap();
        assert_eq!(&first[4..8], &0i32.to_le_bytes());
        assert_eq!(&second[4..8], &1i32.to_le_bytes());

        let mut reader = Full::new();
        assert_eq!(
            reader.read_message(&mut Cursor::new(first)).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(
            reader.read_message(&mut Cursor::new(second)).unwrap(),
            vec![9, 10, 11, 12, 13, 14, 15, 16]
        );
    }

    #[test]
    fn full_detects_seq_no_mismatch() {
        let mut reader = Full::new();
        let frame = full_frame(5, &[1, 2, 3, 4]);
        assert!(matches!(
            reader.read_message(&mut Cursor::new(frame)),
            Err(CodecError::SeqNoMismatch)
        ));
    }

    #[test]
    fn full_detects_crc_mismatch() {
        let mut reader = Full::new();
        let mut frame = full_frame(0, &[1, 2, 3, 4]);
        let last = frame.len() - 1;
        frame[last] ^= 0xff;
        assert!(matches!(
            reader.read_message(&mut Cursor::new(frame)),
            Err(CodecError::CrcMismatch)
        ));
    }

    #[test]
    fn full_rejects_short_frame() {
        let mut reader = Full::new();
        let mut frame = vec![8, 0, 0, 0];
        frame.extend_from_slice(&0i32.to_le_bytes());
        assert!(matches!(
            reader.read_message(&mut Cursor::new(frame)),
            Err(CodecError::InvalidLength(8))
        ));
    }

    #[test]
    fn protocol_error_is_surfaced() {
        // A single-word payload encodes a protocol error whose code is the
        // negation of the trailing word.
        let payload = (-404i32).to_le_bytes();
        let mut buffer = Vec::new();
        Intermediate.write_message(&mut buffer, &payload).unwrap();
        let error = Intermediate
            .read_message(&mut Cursor::new(buffer))
            .unwrap_err();
        assert!(matches!(
            error,
            CodecError::Protocol(ProtocolError { code: 404 })
        ));
    }

    #[test]
    fn large_declared_length_is_read_in_bounded_chunks() {
        // Intermediate frame declaring the maximum length while providing only
        // a few bytes: the reader must fail without allocating 16 MiB first.
        let mut wire = Vec::new();
        wire.extend_from_slice(&(MAX_MESSAGE_SIZE as u32).to_le_bytes());
        wire.extend_from_slice(&[0u8; 64]);
        let error = Intermediate
            .read_message(&mut Cursor::new(wire))
            .unwrap_err();
        assert!(matches!(error, CodecError::Io(_)));
    }

    #[test]
    fn no_header_suppresses_header() {
        let mut codec = NoHeader::new(Intermediate);
        let mut buffer = Vec::new();
        codec.write_header(&mut buffer).unwrap();
        assert!(buffer.is_empty());
        codec.read_header(&mut Cursor::new(Vec::new())).unwrap();
        assert_eq!(codec.obfuscated_tag(), Some(INTERMEDIATE_HEADER));

        let mut frame = Vec::new();
        codec.write_message(&mut frame, &[1, 2, 3, 4]).unwrap();
        // Without the header the frame still has its length prefix.
        assert_eq!(&frame[..4], &4u32.to_le_bytes());
    }

    #[test]
    fn detection() {
        assert_eq!(detect_codec(&[ABRIDGED_HEADER]), CodecKind::Abridged);
        assert_eq!(detect_codec(&INTERMEDIATE_HEADER), CodecKind::Intermediate);
        assert_eq!(
            detect_codec(&PADDED_INTERMEDIATE_HEADER),
            CodecKind::PaddedIntermediate
        );
        assert_eq!(detect_codec(&[0, 0, 0, 0]), CodecKind::Full);
        assert_eq!(detect_codec(&[0x16, 0x03]), CodecKind::Full);
        assert_eq!(CodecKind::Full.obfuscated_tag(), None);
    }

    #[test]
    fn detection_builds_usable_codec() {
        let mut codec = detect_codec(&PADDED_INTERMEDIATE_HEADER).build();
        round_trip(codec.as_mut(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn read_payload_handles_partial_reads() {
        // A reader that yields one byte at a time must still be read fully.
        struct OneByte<'a>(&'a [u8], usize);
        impl Read for OneByte<'_> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.1 >= self.0.len() || buf.is_empty() {
                    return Ok(0);
                }
                buf[0] = self.0[self.1];
                self.1 += 1;
                Ok(1)
            }
        }

        let mut codec = Intermediate;
        let mut frame = Vec::new();
        codec
            .write_message(&mut frame, &[1, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
        let mut source = OneByte(&frame, 0);
        let decoded = codec.read_message(&mut source).unwrap();
        assert_eq!(decoded, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
