# Spec Delta

## Purpose

Defines the verification strategy that establishes wire compatibility between rtd and the
existing Go implementation: shared vectors, byte-level golden tests, and
cross-implementation interop tests.

## ADDED Requirements

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

### Requirement: Deterministic time in tests
Tests that depend on time SHALL use an injectable time source so that message
identifier, salt and timeout behavior can be verified deterministically.

#### Scenario: Time is controlled in a test
- **WHEN** a test advances the injected clock
- **THEN** time-dependent behavior changes deterministically and without real waiting

### Requirement: Negative and boundary testing
The project SHALL include tests for malformed input, boundary sizes, replay,
unsupported layers, and compression bombs.

#### Scenario: Malformed input is rejected safely
- **WHEN** malformed or boundary-sized input is supplied to a decoder
- **THEN** it is rejected or handled without panic and within declared bounds
