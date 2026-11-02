# Spec Delta

## Purpose

Defines WebSocket origin allowlisting.

## ADDED Requirements

### Requirement: Origin allowlist

When an origin allowlist is configured, the edge SHALL accept a WebSocket upgrade only from an
allowed origin and SHALL reject others.

#### Scenario: Allowed origin upgrades
- **WHEN** an upgrade carries an allowed origin
- **THEN** the WebSocket connection is established

#### Scenario: Disallowed origin is rejected
- **WHEN** an upgrade carries an origin not on the list
- **THEN** the edge refuses the upgrade
