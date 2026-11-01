# Spec Delta

## Purpose

Defines how authorization keys and sessions are persisted behind traits so the edge can
store, look up, expire, and release them, with an in-memory backend for tests and
single-process deployments.

## ADDED Requirements

### Requirement: Authorization key store
The system SHALL persist authorization keys through a storage abstraction, keyed by
datacenter and authorization-key identifier. Temporary keys SHALL record an expiry and
SHALL NOT be returned after it.

#### Scenario: Stored key is found by its identifier
- **WHEN** an authorization key is stored and then looked up by its datacenter and identifier
- **THEN** the same key is returned

#### Scenario: Expired temporary key is not returned
- **GIVEN** a temporary authorization key with an expiry
- **WHEN** it is looked up after the expiry
- **THEN** the lookup returns no key

### Requirement: Session store
The system SHALL persist session state keyed by authorization key and session identifier,
including the bound user and negotiated layer, and SHALL remove a session when its
connection is released.

#### Scenario: Session is restored on reconnect
- **WHEN** a session is stored and then looked up by its authorization key and session identifier
- **THEN** its bound user and negotiated layer are returned

#### Scenario: Released session is gone
- **WHEN** a session is released
- **THEN** looking it up by its identifiers returns no session

### Requirement: Fail-closed storage
When the storage backend cannot persist required state, the edge SHALL fail the operation
rather than proceed as if it had been written.

#### Scenario: Write failure blocks confirmation
- **WHEN** the store fails to persist a newly created authorization key
- **THEN** the exchange is not confirmed to the client
