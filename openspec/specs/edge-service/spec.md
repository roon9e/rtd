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

### Requirement: Core-exec bearer credential

The edge SHALL accept an optional core-exec bearer token and, when it is configured, SHALL
present it to the core as gRPC `authorization: Bearer <token>` metadata on every
`CoreExecService` call; when no token is configured it SHALL send no `authorization` metadata.

#### Scenario: A configured token is presented
- **WHEN** the edge is configured with a core-exec token and it calls the core
- **THEN** the request carries `authorization: Bearer <token>` metadata

#### Scenario: An unconfigured token sends no authorization
- **WHEN** the edge is configured without a core-exec token
- **THEN** its requests carry no `authorization` metadata

### Requirement: Container packaging

The edge SHALL be buildable as a container image that runs the edge binary, so it can replace a
gramsrv v2 edge in a compose stack.

#### Scenario: The image runs the edge
- **WHEN** the edge container image is started with a bind address, an RSA key, a core endpoint and token
- **THEN** it serves the MTProto ingress and forwards admitted calls to the core

### Requirement: Admitted-call authorization binding

The edge SHALL bind each admitted call to the authorization key that carried it: the edge-facing
admitted call SHALL carry the authorization-key id and the message id, and the remote core
adapter SHALL transmit both in `DispatchRequest` so a core can resolve and validate the call.

#### Scenario: The call is bound to its authorization key
- **WHEN** the edge dispatches an admitted call to a remote core
- **THEN** the `DispatchRequest` carries the call's authorization-key id and message id, not a default

### Requirement: Negotiated-layer dispatch

The edge SHALL forward the layer the client negotiated (an `invokeWithLayer` selector, inherited
by default) as the `DispatchRequest` profile, and SHALL re-admit in the same default mode, so a
core whose canonical layer differs from the edge's can re-admit the wire.

#### Scenario: A layer-selecting client is dispatched at its layer
- **WHEN** a client selects a layer the core supports
- **THEN** the `DispatchRequest` profile is that layer, not the edge's canonical layer

### Requirement: Wire-proof parity

The edge SHALL transmit a `DispatchRequest` wire proof equal to the proof the core derives from
its own re-admission: the profile, semantic method, wire id, wire size, a lowercase-hex SHA-256
of the wire, the wire-invariance flag, and the effective-profile and profile-evidence marks.

#### Scenario: The proof matches the core's
- **WHEN** an admitted call is dispatched
- **THEN** the core accepts the proof and executes the call

### Requirement: Protocol-accurate wire invariance

The generated wire-invariance predicate SHALL match the reference generator: a method is
wire-invariant only when it is direct in every enabled profile with the same wire id, preflight
plan and result plan in each, so a method whose fields changed across layers (for example
`auth.sendCode`) is not wrongly marked invariant.

#### Scenario: A changed method is not invariant
- **WHEN** a method's request or result plan differs between enabled profiles
- **THEN** it is not reported as wire-invariant

### Requirement: Offline-report authorization binding

The edge SHALL report an offline session with the authorization-key id that owned it.

#### Scenario: An offline session carries its authorization key
- **WHEN** a bound session goes offline and the edge reports it
- **THEN** the `SessionOfflineEvent` carries the session's authorization-key id
