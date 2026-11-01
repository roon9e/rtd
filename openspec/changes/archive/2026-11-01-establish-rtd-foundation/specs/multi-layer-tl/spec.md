# Spec Delta

## Purpose

Defines the runtime behavior of the multi-layer TL model: how inbound requests are
matched to an exact supported layer, how results are encoded back for that layer, and
how unsupported layers are refused without silent coercion.

## ADDED Requirements

### Requirement: Exact-profile admission
The system SHALL admit an inbound request only against an exact supported layer
profile. A layer the system does not support SHALL produce an explicit error and SHALL
NOT be treated as, clamped to, or served with the canonical layer.

#### Scenario: Unsupported layer is refused
- **WHEN** a request selects a layer that is not in the supported set
- **THEN** admission fails with an error identifying the unsupported layer and no canonical substitution occurs

#### Scenario: Supported layer is served exactly
- **WHEN** a request selects a supported layer
- **THEN** the request is decoded using that layer's definition and reported as the effective profile

### Requirement: Profile selection through wrappers
The system SHALL determine the request's layer from the innermost layer-selecting
wrapper. Wrappers that do not select a layer SHALL be consumed without changing the
profile. A request whose selectors disagree SHALL be rejected.

#### Scenario: Layer selector sets the profile
- **WHEN** a request is wrapped by a layer-selecting wrapper naming a supported layer
- **THEN** that layer becomes the effective profile for the request

#### Scenario: Conflicting selectors are rejected
- **WHEN** a request carries two layer-selecting wrappers naming different layers
- **THEN** the request is rejected with a conflict error

### Requirement: Layer-independent semantic identity
Each TL definition SHALL have a semantic identity that is stable across layers and
independent of its wire identifier, so that the same definition is recognizable even
when its wire identifier differs between layers.

#### Scenario: Identity survives an identifier change
- **GIVEN** a definition whose wire identifier differs between two supported layers
- **WHEN** both are resolved
- **THEN** they report the same semantic identity and their respective wire identifiers

### Requirement: Per-profile result and update encoding
Results and updates SHALL be encoded for the negotiated profile of the receiving
session. The system SHALL NOT fall back to a canonical encoding when the negotiated
profile cannot represent a value; it SHALL fail closed instead.

#### Scenario: Result encoded for the client's layer
- **WHEN** a handler returns a value for a session that negotiated a non-canonical supported layer
- **THEN** the value is encoded using that session's layer profile and its wire identifiers

#### Scenario: Unrepresentable value fails closed
- **WHEN** a value cannot be represented in the session's negotiated layer
- **THEN** encoding fails with an explicit error rather than emitting a canonical fallback

### Requirement: Freezing and fan-out of values
The system SHALL support freezing a canonical value once and encoding it into any
supported layer profile, producing a per-profile encoding for each target session.

#### Scenario: One value, several profiles
- **GIVEN** one canonical value and two sessions on different supported layers
- **WHEN** the value is fanned out
- **THEN** each session receives an encoding valid for its own layer

### Requirement: Bounded admission
Admission SHALL enforce declared limits on wire size, vector element count, aggregate
element count and nesting depth, and SHALL bound expansion of compressed payloads.

#### Scenario: Oversized request is rejected
- **WHEN** an inbound request exceeds a declared admission limit
- **THEN** admission fails with an error identifying the exceeded limit

#### Scenario: Compression expansion is bounded
- **WHEN** a compressed payload would expand beyond the declared bound
- **THEN** admission fails rather than allocating beyond the bound

### Requirement: Wire-invariant operations
Operations whose wire representation is identical in every supported layer SHALL be
admissible without a negotiated profile.

#### Scenario: Invariant terminal needs no profile
- **WHEN** a request consists solely of a wire-invariant operation
- **THEN** it is admitted and handled without requiring any layer profile
