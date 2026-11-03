# Spec Delta

## Purpose

Defines one listening port shared by MTProto, WebSocket and HTTP.

## ADDED Requirements

### Requirement: Same-port multiplexing

The edge SHALL serve MTProto, WebSocket and configured HTTP routes on a single listening port,
classifying each connection from a bounded prefix without losing bytes.

#### Scenario: HTTP and MTProto share a port
- **WHEN** an HTTP request and an MTProto connection are made to the same port
- **THEN** each is served on its own path

#### Scenario: Obfuscated connection is not misclassified
- **WHEN** an obfuscated2 connection begins on the shared port
- **THEN** it is handled as MTProto, not HTTP
