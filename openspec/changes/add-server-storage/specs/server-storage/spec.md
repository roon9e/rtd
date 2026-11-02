# Spec Delta

## Purpose

Defines durable server state behind storage traits.

## ADDED Requirements

### Requirement: Storage traits

The server SHALL define storage traits for auth keys, layer evidence and delivery state, and
SHALL ship in-memory and PostgreSQL implementations.

#### Scenario: State survives restart
- **WHEN** a key is saved through the PostgreSQL backend and the process restarts
- **THEN** the key is readable

#### Scenario: In-memory default
- **WHEN** no backend is configured
- **THEN** the server uses the in-memory backend and existing tests still pass
