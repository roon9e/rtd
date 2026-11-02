# Spec Delta

## Purpose

Defines per-method rate limiting and `FLOOD_WAIT` emission.

## ADDED Requirements

### Requirement: Per-method limiting

The server SHALL limit sensitive method classes per identity and SHALL answer
`FLOOD_WAIT_<seconds>` when a limit is exceeded.

#### Scenario: Login burst is limited
- **WHEN** a client exceeds the login rate limit
- **THEN** the server answers `FLOOD_WAIT_<n>` and the client must wait

#### Scenario: Service messages are exempt
- **WHEN** a client sends pings and acks
- **THEN** they are not subject to the method rate limit
