# Spec Delta

## Purpose

Defines a clock-driven server-salt store for the client, refreshed by `get_future_salts`.

## ADDED Requirements

### Requirement: Multiple salts with expiry

The client SHALL store multiple `(valid_until, salt)` entries and SHALL select the newest
unexpired salt for each outgoing message, using the injected clock.

#### Scenario: Newest unexpired salt is selected
- **WHEN** the store holds two salts, one expired and one valid
- **THEN** outgoing messages use the valid salt

### Requirement: Proactive refresh

The client SHALL request `get_future_salts` before the newest salt expires and SHALL store the
returned `future_salts` entries.

#### Scenario: Future salts are adopted
- **WHEN** the server answers `get_future_salts` with `future_salts`
- **THEN** the entries are stored and used once the current salt expires

### Requirement: Bad salt recovery

On `bad_server_salt` the client SHALL adopt the indicated salt and resend the affected message.

#### Scenario: Bad salt is adopted
- **WHEN** the server rejects a message with `bad_server_salt`
- **THEN** the client adopts the new salt and resends the message
