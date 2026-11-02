# Spec Delta

## ADDED Requirements

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
