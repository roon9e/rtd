# Spec Delta

## REMOVED Requirements

### Requirement: Required-field route rejection

## ADDED Requirements

### Requirement: Required-field route defaulting

The system SHALL admit a historical `channels.toggleForum` request that cannot express the
required `sidebar` field and SHALL default `sidebar` to `false`, and SHALL drop `sidebar` when
encoding the canonical method for 225–232.

#### Scenario: Historical toggle-forum defaults sidebar
- **WHEN** a 225–232 request selects `channels.toggleForum`
- **THEN** it is admitted with `sidebar=false` rather than rejected
