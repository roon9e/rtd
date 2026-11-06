# Spec Delta

## Purpose

Records the canonical-layer bump to 233.

## ADDED Requirements

### Requirement: Canonical layer 233

The system SHALL use canonical layer 233, with exact profiles 225 through 233 generated, and
SHALL admit a request only against an exact supported profile.

#### Scenario: Canonical is 233
- **WHEN** the generator runs
- **THEN** it reports canonical 233 with layers 225–233 enabled and accepts the reviewed policy

#### Scenario: New layout flags are canonical-only
- **WHEN** a canonical `channel`/`user` carrying the new sidebar flags is encoded for 225–232
- **THEN** the flags are dropped

### Requirement: Required-field route rejection

The system SHALL reject admission of a historical `channels.toggleForum` request that cannot
express the required `sidebar` decision, and SHALL drop `sidebar` when encoding the canonical
method for 225–232.

#### Scenario: Historical toggle-forum is rejected
- **WHEN** a 225–232 request selects `channels.toggleForum`
- **THEN** admission fails closed rather than defaulting `sidebar`
