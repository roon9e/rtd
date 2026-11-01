# Spec Delta

## ADDED Requirements

### Requirement: Non-direct route execution
The system SHALL admit and execute non-direct routes — retag, rewrite, policy and
profile-only — producing a value valid for the target profile exactly per the reviewed
conversion, and SHALL fail closed only when a route is genuinely unavailable in the
profile.

#### Scenario: Retag route is served
- **WHEN** a request selects a supported layer whose route re-tags the canonical definition
- **THEN** the request is admitted and decoded, and results are encoded with the layer's wire identifier

#### Scenario: Policy route converts
- **WHEN** a request's route requires a reviewed conversion between the layer and canonical shapes
- **THEN** the conversion is applied and the value is valid for the negotiated layer

#### Scenario: Unavailable route fails closed
- **WHEN** a definition has no representation in the selected layer
- **THEN** admission or encoding fails with an explicit error and no canonical fallback is emitted

### Requirement: Per-layer coverage over the enabled set
The system SHALL support every layer the manifest enables, and SHALL serve a request on any
enabled layer whose route is not unavailable, without clamping to the canonical layer.

#### Scenario: Older layer is served
- **WHEN** a request selects an enabled non-canonical layer with a servable route
- **THEN** the request is served on that layer rather than refused or clamped
