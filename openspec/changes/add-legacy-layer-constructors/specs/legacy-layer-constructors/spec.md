# Spec Delta

## Purpose

Defines how a constructor that exists only in an older layer (`ProfileOnly`) is served: a
generated `ProfileOnly` dispatch over the reviewed `old-only` conversion hooks, and the row
adapter that composes them.

## ADDED Requirements

### Requirement: Per-profile legacy codecs

For every `ProfileOnly` constructor an enabled layer needs, the generator SHALL emit a
`ProfileOnly` dispatch in the canonical class's profile codec: it reads the older layer's own
field layout and calls the reviewed `old-only` hook to obtain the canonical class value, and
encoding a canonical value reverses it. There is no canonical route fallback.

#### Scenario: A legacy constructor round trips on its layer

- **WHEN** a legacy constructor defined only in an older layer is decoded and re-encoded for
  that layer
- **THEN** the value survives the round trip through the `old-only` hook

#### Scenario: A legacy constructor is served, not refused

- **WHEN** a `ProfileOnly` route with a registered `old-only` hook is decoded on its older
  layer
- **THEN** it is converted into the canonical class value, and a canonical value of the
  matching type is encoded back into the legacy constructor

#### Scenario: A legacy constructor without a hook fails closed

- **WHEN** a `ProfileOnly` route has no registered `old-only` hook
- **THEN** decoding fails closed with an unsupported-route error

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
