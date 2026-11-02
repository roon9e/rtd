# Spec Delta

## Purpose

Defines how a constructor that exists only in an older layer (`ProfileOnly`) is served: a
generated per-profile field codec, the reviewed `old-only` conversion hooks, and the row
adapter that composes them.

## ADDED Requirements

### Requirement: Per-profile legacy codecs

For every `ProfileOnly` constructor an enabled layer needs, the generator SHALL emit a
per-profile value type and field codec so the constructor can be decoded and re-encoded
without a canonical counterpart.

#### Scenario: A legacy constructor round trips on its layer

- **WHEN** a legacy constructor defined only in an older layer is decoded and re-encoded for
  that layer
- **THEN** the value survives the round trip in the layer's own field layout

#### Scenario: A legacy constructor has no canonical value

- **WHEN** a `ProfileOnly` route is admitted on an older layer
- **THEN** it is decoded into the profile model directly, with no canonical fallback emitted

### Requirement: Reviewed `old-only` conversions

Each `old-only` policy obligation SHALL be executed by an implemented hook that applies
exactly the reviewed conversion between the legacy constructor and the canonical class it
replaced, selected by the obligation's stable key.

#### Scenario: An `old-only` hook applies the reviewed conversion

- **WHEN** a `ProfileOnly` route's obligation resolves to an implemented `old-only` hook
- **THEN** the hook converts the value exactly as recorded in the reviewed policy

#### Scenario: An unrepresentable legacy value fails closed

- **WHEN** a legacy value cannot be represented in the target shape
- **THEN** the conversion fails closed rather than dropping information

### Requirement: Row adapter composes the button hooks

The inline-keyboard row adapter SHALL convert a row of legacy buttons element by element,
using the `old-only` button hooks, and fail closed when a button cannot be represented.

#### Scenario: A representable row round trips

- **WHEN** a row of representable legacy buttons is converted
- **THEN** every button is converted and the row round trips

#### Scenario: An unrepresentable button fails closed

- **WHEN** a row contains a button that cannot be represented
- **THEN** the conversion fails closed
