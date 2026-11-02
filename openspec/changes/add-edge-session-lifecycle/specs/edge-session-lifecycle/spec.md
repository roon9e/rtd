# Spec Delta

## Purpose

Defines logical-session lifecycle, revocation and graceful shutdown.

## ADDED Requirements

### Requirement: Session lifecycle

The edge SHALL track session state, promoting a session on confirmation and revoking a
provisional session that is never confirmed.

#### Scenario: Unconfirmed session is revoked
- **WHEN** a provisional session is not confirmed within the deadline
- **THEN** it is revoked

### Requirement: Graceful shutdown

On shutdown the edge SHALL stop admitting new work and SHALL drain in-flight work within a
bounded time.

#### Scenario: In-flight work drains
- **WHEN** shutdown begins while a call is in flight
- **THEN** the call completes (or the bounded drain times out) before exit
