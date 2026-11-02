# Spec Delta

## MODIFIED Requirements

### Requirement: Present-family non-direct route execution
The system SHALL admit and execute a non-direct route — retag, rewrite or policy — when the
definition is present in both the target layer and the canonical layer, producing a value
valid for the target profile exactly per the reviewed conversion. A route that exists only in
the target layer SHALL be served through its reviewed `old-only` hook when one is registered,
decoded into the canonical class value and encoded back, and SHALL fail closed when no hook
exists. A route with no representation in the target layer SHALL fail closed.

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
- **WHEN** a definition has no route in the selected layer, or exists only in the selected layer with no registered `old-only` hook
- **THEN** admission or encoding fails with an explicit error and no canonical fallback is emitted

#### Scenario: Target-only route is served through its hook
- **WHEN** a definition exists only in the selected layer and a registered `old-only` hook maps it to the canonical class
- **THEN** it is decoded into that canonical value and a matching canonical value is encoded back, with no canonical route fallback
