# Spec Delta

## Purpose

Defines MTProxy-aware dialing that selects the obfuscation layer from a parsed secret.

## ADDED Requirements

### Requirement: Layer selected from the secret

The dialer SHALL select obfuscated2 for a plain or `dd` secret and FakeTLS-over-obfuscated2 for
an `ee` secret, and SHALL apply the derived keys and DC salt.

#### Scenario: `ee` secret negotiates FakeTLS
- **WHEN** the client dials with an `ee` secret
- **THEN** it performs a FakeTLS handshake and then obfuscated2, and frames MTProto above both

#### Scenario: Plain secret uses obfuscated2
- **WHEN** the client dials with a plain secret
- **THEN** only obfuscated2 is applied

### Requirement: Endpoint fallback

The dialer SHALL try the configured endpoints in order and SHALL return the first that
completes its handshake.

#### Scenario: First endpoint fails
- **WHEN** the first endpoint refuses the connection
- **THEN** the dialer tries the next endpoint
