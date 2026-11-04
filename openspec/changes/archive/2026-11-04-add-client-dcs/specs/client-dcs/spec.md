# Spec Delta

## Purpose

Defines datacenter discovery, resolution and migration for the client.

## ADDED Requirements

### Requirement: Datacenter resolution

The client SHALL resolve a datacenter identifier to one or more dialable endpoints and SHALL
select a transport (plain, obfuscated2, FakeTLS or WebSocket) from the options.

#### Scenario: A DC resolves to endpoints
- **WHEN** the client requests a datacenter
- **THEN** it receives an ordered list of endpoints and dials the first that succeeds

### Requirement: Migration

The client SHALL follow `*_MIGRATE_*` errors by switching to the indicated datacenter and SHALL
transfer authorization when required.

#### Scenario: Media migration
- **WHEN** a file method answers `FILE_MIGRATE_2`
- **THEN** the client retries the method on datacenter 2
