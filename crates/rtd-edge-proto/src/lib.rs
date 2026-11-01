//! The gRPC contract between an rtd MTProto edge and a core service.
//!
//! The authoritative definition is `proto/edge.proto`; the message types below
//! are its committed Rust form (the gRPC service is wired in the server stage).
//! The contract is versioned: a core declares [`HelloRequest::contract_version`]
//! and the edge answers with [`handshake`], rejecting an unsupported version
//! rather than misbehaving.
#![warn(missing_docs)]

/// The contract version implemented by this crate.
pub const CONTRACT_VERSION: u32 = 1;

/// Version negotiation request.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct HelloRequest {
    /// The contract version the core speaks.
    #[prost(uint32, tag = "1")]
    pub contract_version: u32,
    /// An identifier for the core service, for diagnostics.
    #[prost(string, tag = "2")]
    pub core_id: String,
}

/// Version negotiation response.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct HelloResponse {
    /// The contract version the edge speaks.
    #[prost(uint32, tag = "1")]
    pub contract_version: u32,
    /// Whether the edge accepted the core's version.
    #[prost(bool, tag = "2")]
    pub accepted: bool,
    /// A human-readable explanation when the version was rejected.
    #[prost(string, tag = "3")]
    pub message: String,
}

/// Identifies one client session at the edge.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct SessionIdentity {
    /// The datacenter the session is connected to.
    #[prost(uint32, tag = "1")]
    pub dc_id: u32,
    /// The auth-key id of the session.
    #[prost(fixed64, tag = "2")]
    pub auth_key_id: u64,
    /// The MTProto session id.
    #[prost(fixed64, tag = "3")]
    pub session_id: u64,
    /// The authenticated user id, or zero when unauthenticated.
    #[prost(uint64, tag = "4")]
    pub user_id: u64,
}

/// One admitted request handed to the core.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct AdmittedCall {
    /// The session the request belongs to.
    #[prost(message, optional, tag = "1")]
    pub session: Option<SessionIdentity>,
    /// The category-qualified TL method name, e.g. `messages.sendMessage`.
    #[prost(string, tag = "2")]
    pub semantic_method: String,
    /// The session's negotiated layer profile.
    #[prost(uint32, tag = "3")]
    pub profile_layer: u32,
    /// The wire constructor id of the request in that profile.
    #[prost(uint32, tag = "4")]
    pub wire_id: u32,
    /// The canonical TL-encoded request.
    #[prost(bytes = "vec", tag = "5")]
    pub canonical_request: Vec<u8>,
}

/// A canonical result returned by the core.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct CallResult {
    /// The canonical TL-encoded result; the edge encodes it per profile.
    #[prost(bytes = "vec", tag = "1")]
    pub canonical_result: Vec<u8>,
}

/// A failure returned by the core.
#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct CallError {
    /// The TL error code.
    #[prost(int32, tag = "1")]
    pub code: i32,
    /// The TL error message.
    #[prost(string, tag = "2")]
    pub message: String,
}

impl HelloRequest {
    /// Answers this request, accepting the contract version when it matches.
    #[must_use]
    pub fn response(&self) -> HelloResponse {
        let accepted = self.contract_version == CONTRACT_VERSION;
        HelloResponse {
            contract_version: CONTRACT_VERSION,
            accepted,
            message: if accepted {
                String::new()
            } else {
                format!(
                    "unsupported contract version {} (expected {CONTRACT_VERSION})",
                    self.contract_version
                )
            },
        }
    }
}

/// Performs the version handshake, rejecting an unsupported core version.
///
/// # Errors
///
/// Returns the edge's [`HelloResponse`] message when the core's version is not
/// [`CONTRACT_VERSION`].
pub fn handshake(request: &HelloRequest) -> Result<HelloResponse, String> {
    let response = request.response();
    if response.accepted {
        Ok(response)
    } else {
        Err(response.message)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdmittedCall, CONTRACT_VERSION, CallResult, HelloRequest, SessionIdentity, handshake,
    };

    #[test]
    fn admit_and_result_round_trip() {
        let call = AdmittedCall {
            session: Some(SessionIdentity {
                dc_id: 2,
                auth_key_id: 0x1122_3344_5566_7788,
                session_id: 0x8877_6655_4433_2211,
                user_id: 42,
            }),
            semantic_method: "messages.sendMessage".to_owned(),
            profile_layer: 232,
            wire_id: 0x1379_4888,
            canonical_request: vec![1, 2, 3, 4],
        };
        let encoded = prost::Message::encode_to_vec(&call);
        let decoded = <AdmittedCall as prost::Message>::decode(encoded.as_slice()).expect("decode");
        assert_eq!(decoded, call);

        let result = CallResult {
            canonical_result: vec![9, 8, 7],
        };
        let encoded = prost::Message::encode_to_vec(&result);
        let decoded =
            <CallResult as prost::Message>::decode(encoded.as_slice()).expect("decode result");
        assert_eq!(decoded, result);
    }

    #[test]
    fn matching_version_is_accepted() {
        let request = HelloRequest {
            contract_version: CONTRACT_VERSION,
            core_id: "gramsrv".to_owned(),
        };
        let response = handshake(&request).expect("accepted");
        assert!(response.accepted);
        assert_eq!(response.contract_version, CONTRACT_VERSION);
    }

    #[test]
    fn mismatched_version_is_rejected() {
        let request = HelloRequest {
            contract_version: CONTRACT_VERSION + 1,
            core_id: "gramsrv".to_owned(),
        };
        assert!(handshake(&request).is_err());
    }
}
