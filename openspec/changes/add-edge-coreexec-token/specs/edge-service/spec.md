# Spec Delta

## Purpose

Defines the edge's credential for its core-exec boundary and how the edge is packaged.

## ADDED Requirements

### Requirement: Core-exec bearer credential

The edge SHALL accept an optional core-exec bearer token and, when it is configured, SHALL
present it to the core as gRPC `authorization: Bearer <token>` metadata on every
`CoreExecService` call; when no token is configured it SHALL send no `authorization` metadata.

#### Scenario: A configured token is presented
- **WHEN** the edge is configured with a core-exec token and it calls the core
- **THEN** the request carries `authorization: Bearer <token>` metadata

#### Scenario: An unconfigured token sends no authorization
- **WHEN** the edge is configured without a core-exec token
- **THEN** its requests carry no `authorization` metadata

### Requirement: Container packaging

The edge SHALL be buildable as a container image that runs the edge binary, so it can replace a
gramsrv v2 edge in a compose stack.

#### Scenario: The image runs the edge
- **WHEN** the edge container image is started with a bind address, an RSA key, a core endpoint and token
- **THEN** it serves the MTProto ingress and forwards admitted calls to the core
