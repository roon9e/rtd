# Spec Delta

## Purpose

Defines metrics, tracing hooks and instance identity.

## ADDED Requirements

### Requirement: Metrics exposition

The server SHALL record counters and histograms for connections, handshakes, admissions,
dispatches and errors, and SHALL expose them over an HTTP endpoint.

#### Scenario: Metrics reflect activity
- **WHEN** a client completes a session
- **THEN** the connection and handshake counters increase and are visible on the endpoint

### Requirement: Instance identity

The edge SHALL have a stable instance identity and SHALL report it with build information at
startup.

#### Scenario: Identity is logged
- **WHEN** the edge starts
- **THEN** it logs its instance id and build info
