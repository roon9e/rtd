# Spec Delta

## Purpose

Defines edge instance identity, build info and runtime metrics.

## ADDED Requirements

### Requirement: Instance identity

The edge SHALL have a stable-per-process instance identity and SHALL report build information at
startup and on a build-info route.

#### Scenario: Build info is exposed
- **WHEN** the build-info route is requested
- **THEN** the edge returns its version and build identifier

### Requirement: Runtime metrics

The edge SHALL expose connection, session, in-flight and byte metrics on the observability
endpoint.

#### Scenario: Runtime metrics reflect traffic
- **WHEN** clients connect and make calls
- **THEN** the corresponding runtime metrics increase
