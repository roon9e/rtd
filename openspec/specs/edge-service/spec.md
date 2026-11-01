# Edge Service Specification

## Purpose

Defines the gRPC boundary that lets a separate core service (initially Go `gramsrv`) use rtd
as its MTProto edge, exchanging raw TL bytes and admitted-call metadata instead of linking
libraries.

## Requirements

### Requirement: Byte-level edge boundary
rtd SHALL expose its MTProto edge to a separate core process over a versioned gRPC
contract. Request and response payloads SHALL cross the boundary as raw TL bytes plus
metadata, and the edge SHALL call the core; the core SHALL NOT require, and rtd SHALL NOT
assume, in-process linking of the two implementations.

#### Scenario: Core drives the edge remotely
- **WHEN** a client connects to an rtd edge and the edge calls its core over the boundary
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

### Requirement: Core owns durable authorization keys
The core SHALL be the durable owner of authorization keys and session-layer evidence. The
edge SHALL save a newly created key to the core before confirming the exchange, and SHALL
resolve a key or layer evidence it does not hold by calling the core and caching it for the
life of the connection.

#### Scenario: New key is saved before confirmation
- **WHEN** the edge completes a key exchange
- **THEN** it saves the key to the core before confirming success

#### Scenario: Unknown key is resolved from the core
- **WHEN** an encrypted frame arrives for an authorization key the edge does not hold
- **THEN** the edge fetches it from the core and proceeds

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

### Requirement: Edge admits, core executes
The edge SHALL perform multi-layer admission locally and send the admitted call — session
identity, semantic method, negotiated profile, wire identifier and canonical bytes — to the
core, which executes it and returns a canonical result the edge encodes for the session's
layer.

#### Scenario: Admitted call reaches the core
- **WHEN** the edge admits a request
- **THEN** the core receives the admitted call and returns a result the edge encodes for the session's layer

### Requirement: Interchangeable core implementations
The edge SHALL reach its core through one abstraction satisfied by both the gRPC adapter
and an in-process implementation, so the edge runs with either without changing observable
protocol behavior.

#### Scenario: In-process core serves a request
- **WHEN** the edge runs with an in-process core implementation
- **THEN** requests are served with the same observable behavior as over the gRPC adapter

### Requirement: Core unavailable
When the core cannot be reached or fails, the edge SHALL return a TL error to the client
and SHALL keep the session alive.

#### Scenario: Core failure is surfaced
- **WHEN** the core is unreachable for an admitted request
- **THEN** the client receives a TL error and the connection remains usable
