# client-runtime Specification

## Purpose
Defines the MTProto 2.0 client runtime: how a client composes the transport, completes the
auth-key exchange, runs an encrypted session, and invokes canonical-layer methods.

## Requirements

### Requirement: Transport composition

The client SHALL connect to a server over TCP with a `rtd-proto` codec and, when a proxy
secret is configured, an obfuscation layer, exposing framed bytes behind a transport
abstraction that an in-memory duplex can substitute.

#### Scenario: Frames move through the transport
- **WHEN** the client writes a frame to the transport
- **THEN** the peer reads the same bytes, and vice versa

#### Scenario: Codec and obfuscation are honored
- **WHEN** the client is constructed with a codec kind and an optional secret
- **THEN** outgoing frames are framed (and obfuscated) accordingly and incoming frames are unframed (and deobfuscated)

### Requirement: Authorization-key exchange

The client SHALL complete the MTProto 2.0 requester auth-key exchange against the server and
derive the same authorization key, session identifier and server salt as the server.

#### Scenario: Client and server derive the same key
- **WHEN** the client's exchange runs against a server's exchange over a duplex
- **THEN** both sides report the same authorization key, server salt and nonzero session identifier

### Requirement: Encrypted session

The client SHALL frame outbound messages with MTProto 2.0 (message id, seqno, salt, session
id, padding, AES-IGE) using `Side::Client`, and SHALL decrypt and validate inbound server
messages, adopting a refreshed salt from a `bad_server_salt` notification.

#### Scenario: Encrypted round trip
- **WHEN** the client sends an encrypted message and the peer replies encrypted
- **THEN** the client decrypts the reply and validates its session id

#### Scenario: Stale salt is adopted
- **WHEN** the server reports `bad_server_salt` with a new salt
- **THEN** the client updates its salt and resends

### Requirement: RPC invocation

The client SHALL correlate a reply to its request by the request message id, unwrap message
containers and gzip, acknowledge server messages that require it, and return the decoded
canonical value; an `rpc_error` SHALL be surfaced as an error.

#### Scenario: A request returns its result
- **WHEN** the client invokes a method and the server answers with an `rpc_result` for that request
- **THEN** the client returns the decoded `rtd_tg::Object`

#### Scenario: An error result is surfaced
- **WHEN** the server answers with an `rpc_error`
- **THEN** the client returns the corresponding error rather than a value

### Requirement: Session establishment

The client SHALL establish its first session by invoking `help.getConfig` (or the caller's
query) wrapped in `invokeWithLayer` and `initConnection` at the canonical layer, and SHALL
return the inner result.

#### Scenario: First call establishes the session
- **WHEN** the client makes its first call after the exchange
- **THEN** the request is sent wrapped in `invokeWithLayer`/`initConnection` at layer 232 and the inner result is returned
