# Spec Delta

## Purpose

Defines decoding of bare-vector RPC results for the client.

## ADDED Requirements

### Requirement: Bare-vector results

The client SHALL decode an RPC result that is a bare `Vector<T>` into a vector of objects, and
SHALL expose it alongside constructor results.

#### Scenario: Vector result decodes
- **WHEN** a method returns a bare vector
- **THEN** the result is a vector of the decoded objects

#### Scenario: Constructor result still decodes
- **WHEN** a method returns a constructor
- **THEN** the result is that single object
