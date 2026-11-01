# Edge Service Specification

## Purpose

Defines the gRPC boundary that lets a separate core service (initially Go `gramsrv`) use rtd
as its MTProto edge, exchanging raw TL bytes and admitted-call metadata instead of linking
libraries.

## Requirements

### Requirement: Byte-level edge boundary
rtd SHALL expose its MTProto edge to a separate core process over a versioned gRPC
contract. Request and response payloads SHALL cross the boundary as raw TL bytes plus
metadata; the core SHALL NOT require, and rtd SHALL NOT assume, in-process linking of the
two implementations.

#### Scenario: Core drives the edge remotely
- **WHEN** a core service connects to an rtd edge over the boundary and opens a client connection
- **THEN** the edge performs the protocol work and exchanges TL bytes with the core across the boundary

### Requirement: Admitted-call metadata
For each inbound request, the edge SHALL deliver to the core the session identity, the
admitted semantic method, the negotiated layer profile, the wire identifier, and the
canonical request bytes.

#### Scenario: Core learns the negotiated layer
- **WHEN** a request is admitted for a session that negotiated a non-canonical layer
- **THEN** the core receives the semantic method, the negotiated profile and the wire identifier with the canonical request bytes

### Requirement: Per-profile result encoding at the edge
The core SHALL return canonical result values, and the edge SHALL encode them for the
session's negotiated layer. If a value cannot be represented in that layer, the edge SHALL
fail closed rather than emitting a canonical fallback.

#### Scenario: Core result is encoded for the client's layer
- **WHEN** the core returns a canonical result for a session on a supported layer
- **THEN** the edge encodes it using that layer and writes it to the client

#### Scenario: Unrepresentable result fails closed
- **WHEN** the core returns a value that cannot be represented in the session's layer
- **THEN** the edge reports an error and does not emit a canonical fallback

### Requirement: Edge owns keys and sessions
The edge SHALL own authorization keys and session state through its storage traits, and
SHALL expose lookup and lifecycle operations to the core so the core does not hold them
directly.

#### Scenario: Core resolves a session through the edge
- **WHEN** the core needs session or authorization-key information to run business logic
- **THEN** it obtains it from the edge rather than from its own storage

### Requirement: Lifecycle and routing events
The edge SHALL surface lifecycle and routing events to the core, including session
created/closed, negotiated-layer changes, and datacenter migration or routing decisions.

#### Scenario: Layer change is announced
- **WHEN** a session's negotiated layer changes
- **THEN** the core is notified of the new negotiated layer for that session

### Requirement: Error propagation across the boundary
A failure reported by the core for a request SHALL be delivered to the client as a TL error
with the code and message the core supplied.

#### Scenario: Core error reaches the client
- **WHEN** the core returns an error for a request
- **THEN** the client receives the corresponding TL error code and message

### Requirement: Versioned contract
The edge contract SHALL be explicitly versioned so the edge and the core can be upgraded
independently, and a version mismatch SHALL be detectable at connection time.

#### Scenario: Mismatched versions are detected
- **WHEN** a core connects with an unsupported contract version
- **THEN** the edge refuses the connection with a version error rather than misbehaving
