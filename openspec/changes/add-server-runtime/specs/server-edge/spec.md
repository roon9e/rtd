# Spec Delta

## ADDED Requirements

### Requirement: Encrypted frame acceptance
After a key exchange, the edge SHALL accept encrypted frames by their authorization-key
identifier, decrypt and process them, and SHALL answer an unknown identifier with an
authorization-key-not-found error rather than dropping the connection.

#### Scenario: Known key is accepted
- **WHEN** an encrypted frame arrives carrying a stored authorization-key identifier
- **THEN** the edge decrypts and processes the message

#### Scenario: Unknown key is refused
- **WHEN** an encrypted frame carries an authorization-key identifier the edge does not hold
- **THEN** the edge answers with an authorization-key-not-found error

### Requirement: MTProxy ingress
The edge SHALL accept MTProxy connections that use FakeTLS or obfuscated2 framing with a
configured secret, unwrap them to the MTProto stream, and reject a connection whose
obfuscation does not authenticate with the secret.

#### Scenario: FakeTLS connection with a valid secret is accepted
- **WHEN** a client opens a FakeTLS connection using the configured secret
- **THEN** the edge completes the handshake and serves the unwrapped MTProto stream

#### Scenario: Wrong secret is rejected
- **WHEN** a client opens an MTProxy connection whose obfuscation does not match the configured secret
- **THEN** the edge rejects it
