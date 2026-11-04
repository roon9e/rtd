# Spec Delta

## Purpose

Defines per-datacenter connection pooling for the client.

## ADDED Requirements

### Requirement: Connection pooling

The client SHALL maintain a pool of connections per datacenter, bounded by a maximum, and SHALL
reuse idle connections before opening new ones.

#### Scenario: Idle connection is reused
- **WHEN** a connection is released and another call arrives
- **THEN** the released connection is reused rather than a new one opened

### Requirement: Dead connection replacement

The pool SHALL replace a dead connection and retry the affected request once when the failure is
a connection error.

#### Scenario: Dead connection is replaced
- **WHEN** a pooled connection is dead and a request is issued
- **THEN** the pool opens a replacement and the request completes
