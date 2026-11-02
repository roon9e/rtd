# Spec Delta

## ADDED Requirements

### Requirement: Old-only constructors are converted through reviewed hooks
A constructor that exists only in an older layer SHALL be converted to and from the canonical
class it replaced by a reviewed `old-only` hook, selected by its stable key and field order.
The generation gate SHALL reject an `old-only` adapter obligation with no registered hook and
a registered hook that no obligation references.

#### Scenario: A target-only constructor applies its reviewed hook
- **WHEN** a `ProfileOnly` route's obligation resolves to a registered `old-only` hook
- **THEN** the hook converts the constructor exactly as recorded in the reviewed policy, in the schema's field order

#### Scenario: A missing or stale old-only hook fails generation
- **WHEN** an `old-only` adapter obligation has no registered hook, or a registered hook is referenced by no obligation
- **THEN** generation fails with an explicit error rather than failing closed at runtime
