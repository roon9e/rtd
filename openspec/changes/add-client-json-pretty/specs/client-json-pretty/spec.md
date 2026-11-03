# Spec Delta

## Purpose

Defines TL JSON coding, JSON invoke and pretty-printing.

## ADDED Requirements

### Requirement: TL JSON

The client SHALL encode and decode TL objects to and from JSON using the constructor name in an
`@type` field.

#### Scenario: Object round-trips
- **WHEN** a TL object is encoded to JSON and decoded back
- **THEN** the decoded value equals the original

### Requirement: JSON invoke

The client SHALL accept a JSON method request, invoke it, and return the JSON result or error.

#### Scenario: JSON invoke succeeds
- **WHEN** a valid JSON method request is invoked
- **THEN** the JSON response encodes the result constructor
