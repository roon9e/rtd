# Spec Delta

## REMOVED Requirements

### Requirement: Edge owns keys and sessions
**Reason**: Matching gramsrv v2, durable authorization keys and session-layer evidence are owned by the core; the edge holds only transient state.
**Migration**: The core is the durable owner; the edge saves a newly created key to the core and resolves keys or evidence it does not hold by calling the core. See the added "Core owns durable authorization keys" requirement.

## MODIFIED Requirements

### Requirement: Byte-level edge boundary
rtd SHALL expose its MTProto edge to a separate core process over a versioned gRPC
contract. Request and response payloads SHALL cross the boundary as raw TL bytes plus
metadata, and the edge SHALL call the core; the core SHALL NOT require, and rtd SHALL NOT
assume, in-process linking of the two implementations.

#### Scenario: Core drives the edge remotely
- **WHEN** a client connects to an rtd edge and the edge calls its core over the boundary
- **THEN** the edge performs the protocol work and exchanges TL bytes with the core across the boundary

## ADDED Requirements

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
