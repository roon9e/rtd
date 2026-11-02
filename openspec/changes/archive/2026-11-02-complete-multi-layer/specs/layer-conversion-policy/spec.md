# Spec Delta

## Purpose

Defines the reviewed per-layer conversion behavior that non-direct routes execute: the
policy hooks and the mechanical conversions that map between canonical and older-layer
shapes, and the coverage gate that keeps every audited obligation implemented.

## ADDED Requirements

### Requirement: Reviewed policy hooks
Each policy obligation SHALL be executed by an implemented hook that applies exactly the
reviewed conversion for that obligation, selected by the obligation's stable key.

#### Scenario: Hook applies the reviewed conversion
- **WHEN** a route's obligation resolves to an implemented hook
- **THEN** the hook converts the value exactly as recorded in the reviewed policy

#### Scenario: Unknown hook is rejected
- **WHEN** an obligation names a hook that is not implemented
- **THEN** the conversion fails closed rather than guessing

### Requirement: Mechanical conversions
The system SHALL apply the mechanical conversion classes exactly: drop discards a
profile-only field, reject fails the conversion, reject-if-present fails only when the
field is present, default substitutes the recorded default, and project maps a field
between shapes.

#### Scenario: Reject-if-present passes when the field is absent
- **WHEN** a reject-if-present field is absent
- **THEN** the conversion succeeds

#### Scenario: Reject-if-present fails when the field is present
- **WHEN** a reject-if-present field is present
- **THEN** the conversion fails closed

#### Scenario: Drop discards the field
- **WHEN** a drop conversion applies
- **THEN** the field is omitted from the output

#### Scenario: Default substitutes the recorded value
- **WHEN** a default conversion applies
- **THEN** the recorded default replaces the absent value

### Requirement: Unavailable routes are the absence of a conversion
A route with no representation in the target profile SHALL be unavailable and SHALL fail
closed; the absence of a body is the action, not a generation error.

#### Scenario: Absent route fails closed at runtime
- **WHEN** a value has no route in the negotiated profile
- **THEN** encoding fails closed with an explicit error
