//! W3C trace-context wrapper over the canonical Telegram API.
//!
//! `invokeWithTrace` is a generic wrapper (`{X:Type} ... query:!X = X`): it
//! carries a W3C `traceparent`/`tracestate`/`baggage` around an arbitrary
//! canonical query. Because the payload is an arbitrary canonical method rather
//! than a trace-local type, the wrapper is defined here over
//! [`rtd_tg::Method`] instead of being emitted by the value-model generator.
#![warn(missing_docs)]

use rtd_bin::{Error, Reader, Writer};
use rtd_tg::{DecodeLimits, Decoder, Method};

/// The `invokeWithTrace` wire constructor identifier.
pub const INVOKE_WITH_TRACE_TYPE_ID: u32 = 0x104b_e9ab;

/// A canonical query wrapped with W3C trace context.
#[derive(Debug, Clone, PartialEq)]
pub struct InvokeWithTrace {
    /// The W3C `traceparent` header, when present.
    pub trace_parent: Option<String>,
    /// The W3C `tracestate` header, when present.
    pub trace_state: Option<String>,
    /// The W3C `baggage` header, when present.
    pub baggage: Option<String>,
    /// The wrapped canonical query.
    pub query: Method,
}

impl InvokeWithTrace {
    /// The wire constructor identifier.
    pub const TYPE_ID: u32 = INVOKE_WITH_TRACE_TYPE_ID;

    /// Encodes the value, including its constructor identifier.
    pub fn encode(&self, writer: &mut Writer) {
        writer.put_u32(Self::TYPE_ID);
        self.encode_body(writer);
    }

    /// Encodes the value without its constructor identifier.
    pub fn encode_body(&self, writer: &mut Writer) {
        let mut flags = 0u32;
        if self.trace_parent.is_some() {
            flags |= 1 << 1;
        }
        if self.trace_state.is_some() {
            flags |= 1 << 2;
        }
        if self.baggage.is_some() {
            flags |= 1 << 3;
        }
        writer.put_u32(flags);
        if let Some(value) = &self.trace_parent {
            writer.put_string(value);
        }
        if let Some(value) = &self.trace_state {
            writer.put_string(value);
        }
        if let Some(value) = &self.baggage {
            writer.put_string(value);
        }
        self.query.encode(writer);
    }

    /// Decodes a value, reading and checking its identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when the identifier is wrong or the payload is
    /// malformed.
    pub fn decode(reader: &mut Reader<'_>) -> rtd_bin::Result<Self> {
        let id = reader.take_u32()?;
        Self::decode_with_id(reader, id)
    }

    /// Decodes a value whose identifier has already been read.
    ///
    /// # Errors
    ///
    /// Returns an error when the identifier is wrong or the payload is
    /// malformed.
    pub fn decode_with_id(reader: &mut Reader<'_>, id: u32) -> rtd_bin::Result<Self> {
        if id != Self::TYPE_ID {
            return Err(Error::UnexpectedId {
                id,
                expected: Self::TYPE_ID,
            });
        }
        let flags = reader.take_u32()?;
        let trace_parent = if flags & (1 << 1) != 0 {
            Some(reader.take_string()?.to_owned())
        } else {
            None
        };
        let trace_state = if flags & (1 << 2) != 0 {
            Some(reader.take_string()?.to_owned())
        } else {
            None
        };
        let baggage = if flags & (1 << 3) != 0 {
            Some(reader.take_string()?.to_owned())
        } else {
            None
        };
        // The wrapped query is the final field, so decode it from the remaining
        // bytes with the canonical decoder.
        let mut decoder = Decoder::new(reader.peek_rest(), DecodeLimits::default());
        let query = Method::decode(&mut decoder)?;
        Ok(Self {
            trace_parent,
            trace_state,
            baggage,
            query,
        })
    }
}
