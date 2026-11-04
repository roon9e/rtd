# Spec Delta

## Purpose

Defines automated interoperability and external-peer checks.

## ADDED Requirements

### Requirement: External peer gate in CI

CI SHALL build the official Rust MTProto engine's test server from a pinned ref and run the
`peer_gate` tests against it, without linking or vendoring the engine.

#### Scenario: Peer gate runs
- **WHEN** a push or pull request is tested
- **THEN** the exchange and client peer gates run against the built test server

### Requirement: Vector freshness

CI SHALL regenerate the Go interop vectors and fail if the committed vectors differ.

#### Scenario: Vectors are stale
- **WHEN** the committed vectors no longer match the reference program's output
- **THEN** the job fails
