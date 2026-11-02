# Spec Delta

## ADDED Requirements

### Requirement: Non-direct route body emission
For every non-direct route the generator SHALL emit an executable body derived from its
conversion plan, replacing the not-emitted stub, and SHALL deduplicate identical bodies by
digest so a shared conversion is emitted once.

#### Scenario: A retag route is emitted
- **WHEN** a route is classified as retag between two enabled layers
- **THEN** the generated sidecar contains a body that performs the retag

#### Scenario: Identical bodies are shared
- **WHEN** two routes have identical conversion bodies
- **THEN** the generator emits one shared plan referenced by both

### Requirement: Hook coverage gate
The generator SHALL reference an implemented hook for every policy obligation and SHALL
fail generation when an obligation has no hook or a hook is not referenced by any
obligation (stale).

#### Scenario: Missing hook fails generation
- **WHEN** a policy obligation has no implemented hook
- **THEN** generation fails and names the obligation

#### Scenario: Stale hook fails generation
- **WHEN** an implemented hook is not referenced by any policy obligation
- **THEN** generation fails and reports it as stale
