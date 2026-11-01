# Interop Testing Specification

## Purpose

Defines the verification strategy that establishes wire compatibility between rtd, the Go
implementation, and the official Rust MTProto engine: shared vectors, byte-level golden
tests, cross-implementation interop tests, and an external-peer harness that is an
acceptance gate.

## Requirements

### Requirement: Shared cryptographic and protocol vectors
The project SHALL retain and run a shared set of vectors for cryptography and wire
framing derived from the reference implementation and public protocol sources, so that
behavioral changes are detected.

#### Scenario: Vector mismatch fails the build
- **WHEN** an implementation change alters an output that is covered by a shared vector
- **THEN** the test fails and reports the differing vector

### Requirement: Byte-level framing goldens
The project SHALL include byte-level golden tests for every supported transport framing
and for message container and compression encoding.

#### Scenario: Encoded bytes match the golden
- **WHEN** a message is encoded with a supported framing
- **THEN** the resulting bytes equal the recorded golden bytes

### Requirement: Cross-implementation interop
The project SHALL run interop tests in which the reference Go implementation and rtd
communicate over the real wire protocol, and these tests SHALL be part of the change's
acceptance.

#### Scenario: Reference client against rtd server
- **WHEN** a reference Go client performs a key exchange and a basic request against an rtd server
- **THEN** the exchange and the request succeed over the wire

#### Scenario: Older-layer client against rtd server
- **WHEN** a reference client negotiates a supported non-canonical layer with an rtd server
- **THEN** the request succeeds and the response is valid for that layer

### Requirement: Official-engine interoperability is an acceptance gate
Interoperability with the official Rust MTProto engine SHALL be an acceptance requirement:
the milestone SHALL NOT be accepted unless interop tests against that engine pass. A failure
SHALL name the exact divergence instead of being skipped.

#### Scenario: Milestone acceptance requires official-engine interop
- **WHEN** the milestone is evaluated for acceptance
- **THEN** passing interop against the official engine is required, and a skipped or failing check blocks acceptance

#### Scenario: Divergence is diagnosed, not hidden
- **WHEN** interop against the official engine fails
- **THEN** the failing check reports which message, framing or behaviour diverged

### Requirement: External-peer harness
The project SHALL obtain the official engine's published test server (and, where needed, its
C-ABI library) as an external peer and exchange real protocol traffic with it. The official
engine SHALL NOT be copied into rtd or taken as a build dependency.

#### Scenario: rtd handshakes with the official test server
- **WHEN** rtd's client-side code path connects to the official engine's test server
- **THEN** the authorization-key exchange completes and at least one request is answered over the wire

#### Scenario: No dependency on the official engine
- **WHEN** the rtd workspace is built
- **THEN** no crate or build step requires the official engine's source or crate

### Requirement: Full-session external-peer gate
The external-peer gate SHALL establish a complete encrypted session against the official
engine's test server — the authorization-key exchange followed by encrypted service
traffic — and SHALL require a `pong` response to a `ping`, in addition to the handshake.

#### Scenario: Encrypted ping is answered
- **WHEN** rtd completes a key exchange with the official test server and sends an encrypted ping
- **THEN** the server answers with a matching pong and the gate passes

### Requirement: Deterministic time and randomness in tests
Tests that depend on time or randomness SHALL use injected sources, so that message
identifier, salt, key derivation and timeout behavior can be verified deterministically.

#### Scenario: Time is controlled in a test
- **WHEN** a test advances the injected clock
- **THEN** time-dependent behavior changes deterministically and without real waiting

#### Scenario: Randomness is controlled in a test
- **WHEN** a test supplies a fixed random source to the protocol core
- **THEN** the same input produces byte-identical output across runs

### Requirement: Negative and boundary testing
The project SHALL include tests for malformed input, boundary sizes, replay,
unsupported layers, and compression bombs.

#### Scenario: Malformed input is rejected safely
- **WHEN** malformed or boundary-sized input is supplied to a decoder
- **THEN** it is rejected or handled without panic and within declared bounds

### Requirement: In-process end-to-end session
The project SHALL include an end-to-end test in which a test client completes a key
exchange and an RPC against an rtd edge backed by a fake core, over each supported ingress
transport.

#### Scenario: Session over a plain TCP framing
- **WHEN** a test client connects over a supported TCP framing, exchanges a key and calls a built-in method
- **THEN** the exchange and the call succeed over the wire

#### Scenario: Session over an MTProxy transport
- **WHEN** a test client connects through MTProxy obfuscation, exchanges a key and calls a built-in method
- **THEN** the exchange and the call succeed over the wire
