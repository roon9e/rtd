# Spec Delta

## ADDED Requirements

### Requirement: Present-family non-direct route execution
The system SHALL admit and execute a non-direct route — retag, rewrite or policy — when the
definition is present in both the target layer and the canonical layer, producing a value
valid for the target profile exactly per the reviewed conversion, and SHALL fail closed when
a route has no representation in the target layer or exists only in the target layer.

#### Scenario: Retag route is served
- **WHEN** a request selects an enabled layer whose route re-tags a canonical definition present in that layer
- **THEN** the request is admitted and decoded, and results are encoded with the layer's wire identifier

#### Scenario: Rewrite route re-encodes
- **WHEN** a route's body layout differs between the layer and canonical for the same definition
- **THEN** the value is re-encoded in the layer's field layout

#### Scenario: Policy route converts
- **WHEN** a request's route requires a reviewed conversion between the layer and canonical shapes
- **THEN** the conversion is applied and the value is valid for the negotiated layer

#### Scenario: Absent or target-only route fails closed
- **WHEN** a definition has no route in the selected layer, or exists only in the selected layer
- **THEN** admission or encoding fails with an explicit error and no canonical fallback is emitted

### Requirement: Per-layer coverage over the enabled set
The system SHALL support every present-in-both definition on any enabled layer whose route is
not unavailable, without clamping to the canonical layer.

#### Scenario: Older layer is served
- **WHEN** a request selects an enabled non-canonical layer with a servable route
- **THEN** the request is served on that layer rather than refused or clamped
