//! TL binary serialization primitives.
#![warn(missing_docs)]
//!
//! This crate implements the byte- and value-level encoding used by MTProto.
//! It provides two halves with explicit ownership:
//!
//! - [`Writer`] — an owned, growable encoding target. Encoding appends into it;
//!   the result is inspected with [`Writer::as_slice`] or taken with
//!   [`Writer::into_vec`]. [`Writer::clear`] and [`Writer::patch_u32`] are the
//!   reset and in-place patch escape hatches.
//! - [`Reader`] — a borrowed decoding cursor over `&[u8]`. Byte-string and
//!   string values are returned as slices borrowed from the input, so decoding
//!   is zero-copy and never allocates for those types.
//!
//! All multi-byte integers are little-endian. The wire constants and
//! fixed-width integer types live in [`types`].

pub mod error;
pub mod fields;
pub mod reader;
pub mod types;
pub mod writer;

pub use error::{Error, Result};
pub use fields::Fields;
pub use reader::{MAX_PAYLOAD_LEN, Reader};
pub use types::{
    FIRST_LONG_STRING, Int128, Int256, MAX_LONG_STRING, MAX_SMALL_STRING, PREALLOCATE_LIMIT,
    TYPE_BOOL_FALSE, TYPE_BOOL_TRUE, TYPE_BYTES, TYPE_DOUBLE, TYPE_INT, TYPE_LONG, TYPE_STRING,
    TYPE_VECTOR, WORD, padded_len, padding_for, preallocate_len,
};
pub use writer::Writer;
