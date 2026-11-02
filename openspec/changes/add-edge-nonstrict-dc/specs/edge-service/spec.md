# Spec Delta

## ADDED Requirements

### Requirement: Datacenter-label acceptance

A single-backend edge SHALL be configurable to accept the datacenter label a client selects during
the key exchange rather than requiring it to equal the served datacenter, matching the reference
edge's `strict_dc_check: false`. When strict acceptance is enabled the edge SHALL reject a
mismatched label.

#### Scenario: A foreign label is accepted in non-strict mode
- **WHEN** the edge runs non-strict and a client requests a different datacenter label
- **THEN** the exchange completes and the client derives the same authorization key

#### Scenario: A mismatched label is rejected in strict mode
- **WHEN** the edge runs strict and a client requests a different datacenter label
- **THEN** the exchange fails with a datacenter mismatch

### Requirement: Non-strict acceptance is not multi-DC routing

Non-strict label acceptance SHALL be treated as a single-backend compatibility mode and SHALL NOT
be regarded as datacenter routing: when a multi-DC runtime exists, the label selects the backend
and is part of the key and session identity, and an unknown label SHALL be routed or refused
rather than served by the local datacenter.

#### Scenario: A multi-DC runtime maps the label
- **WHEN** a multi-DC runtime receives a datacenter label
- **THEN** it selects the backend for that label (or refuses an unknown one), and the label remains part of the key/session identity
