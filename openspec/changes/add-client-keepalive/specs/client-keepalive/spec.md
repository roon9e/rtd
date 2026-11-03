# Spec Delta

## Purpose

Defines client keepalive: periodic pings, dead-connection detection and reconnection with retry.

## ADDED Requirements

### Requirement: Periodic ping

The client SHALL send a ping at a configurable interval and SHALL treat a missing `pong` as a
dead connection.

#### Scenario: Missed pong marks the connection dead
- **WHEN** no `pong` arrives within the ping timeout
- **THEN** the client considers the connection dead and starts reconnecting

### Requirement: Reconnect with backoff

The client SHALL reconnect with capped exponential backoff, reusing the persisted session data
where possible.

#### Scenario: Reconnect reuses the session
- **WHEN** the connection is re-established after a drop
- **THEN** the client reuses the stored auth key and does not repeat the full login flow

### Requirement: Retry unprocessed requests

After a reconnect the client SHALL resend requests that were not acknowledged, and SHALL NOT
resend requests the server already acknowledged.

#### Scenario: Unacknowledged request is retried
- **WHEN** a request was sent but not acknowledged before the connection dropped
- **THEN** it is resent on the new connection
