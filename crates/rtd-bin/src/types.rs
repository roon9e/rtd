//! TL wire constants and fixed-width integer types.
//!
//! All multi-byte integers on the TL wire are little-endian. The constants in
//! this module are the constructor identifiers and limits used by the
//! serialization primitives.

/// Size in bytes of a TL "word".
pub const WORD: usize = 4;

/// Constructor identifier of the `int` boxed type.
pub const TYPE_INT: u32 = 0xa850_9bda;
/// Constructor identifier of the `long` boxed type.
pub const TYPE_LONG: u32 = 0x2207_6cba;
/// Constructor identifier of the `double` boxed type.
pub const TYPE_DOUBLE: u32 = 0x2210_c154;
/// Constructor identifier of the `string` boxed type.
pub const TYPE_STRING: u32 = 0xb528_6e24;
/// Constructor identifier of the `vector` boxed type.
pub const TYPE_VECTOR: u32 = 0x1cb5_c415;
/// Constructor identifier of the `bytes` boxed type.
pub const TYPE_BYTES: u32 = 0xe937_bb82;
/// Constructor identifier of `boolTrue`.
pub const TYPE_BOOL_TRUE: u32 = 0x9972_75b5;
/// Constructor identifier of `boolFalse`.
pub const TYPE_BOOL_FALSE: u32 = 0xbc79_9737;

/// Largest byte length that uses the single-byte length prefix.
pub const MAX_SMALL_STRING: usize = 253;
/// Marker byte introducing the long (three-byte) length form.
pub const FIRST_LONG_STRING: u8 = 254;
/// Largest byte length representable by the long length form (`2^24 - 1`).
pub const MAX_LONG_STRING: usize = (1 << 24) - 1;

/// Upper bound applied when pre-allocating a vector from an untrusted length.
///
/// Decoding a vector must not trust the declared element count for allocation;
/// generated decoders clamp with [`preallocate_len`].
pub const PREALLOCATE_LIMIT: usize = 1024;

/// Returns the number of padding bytes needed after `len` payload bytes to
/// reach a 4-byte boundary.
#[must_use]
pub const fn padding_for(len: usize) -> usize {
    let rem = len % WORD;
    if rem == 0 { 0 } else { WORD - rem }
}

/// Rounds `len` up to the nearest multiple of [`WORD`].
#[must_use]
pub const fn padded_len(len: usize) -> usize {
    len + padding_for(len)
}

/// Clamps an untrusted vector element count to [`PREALLOCATE_LIMIT`].
#[must_use]
pub const fn preallocate_len(len: usize) -> usize {
    if len > PREALLOCATE_LIMIT {
        PREALLOCATE_LIMIT
    } else {
        len
    }
}

/// The TL `int128` type: 16 raw little-endian bytes.
///
/// The wire representation is little-endian. Crypto routines that need a
/// big-endian integer interpretation (RSA, Diffie-Hellman) reinterpret the
/// bytes explicitly at the call site.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Int128([u8; 16]);

impl Int128 {
    /// Length of the type in bytes.
    pub const LEN: usize = 16;

    /// Wraps 16 raw bytes.
    #[must_use]
    pub const fn from_array(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Returns the raw bytes.
    #[must_use]
    pub const fn as_array(&self) -> &[u8; 16] {
        &self.0
    }
}

impl AsRef<[u8]> for Int128 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl core::fmt::Debug for Int128 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Int128(")?;
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        write!(f, ")")
    }
}

/// The TL `int256` type: 32 raw little-endian bytes.
///
/// See [`Int128`] for the endianness note.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Int256([u8; 32]);

impl Int256 {
    /// Length of the type in bytes.
    pub const LEN: usize = 32;

    /// Wraps 32 raw bytes.
    #[must_use]
    pub const fn from_array(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the raw bytes.
    #[must_use]
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl AsRef<[u8]> for Int256 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl core::fmt::Debug for Int256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Int256(")?;
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        write!(f, ")")
    }
}
