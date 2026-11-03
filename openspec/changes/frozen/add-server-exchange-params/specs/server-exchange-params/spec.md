# Spec Delta

## Purpose

Defines production Diffie-Hellman parameters for the server exchange.

## ADDED Requirements

### Requirement: Fresh parameters per handshake

The server SHALL generate a fresh, valid `pq` and `p`/`g` for each key exchange, using the
injected randomness, and SHALL NOT reuse the fixed test parameters outside tests.

#### Scenario: Client accepts the parameters
- **WHEN** the server answers `req_pq`/`req_DH_params` with generated parameters
- **THEN** the client validates them and the exchange completes

#### Scenario: Test mode is explicit
- **WHEN** the edge is started without the test flag
- **THEN** it uses the production generator
