# Spec Delta

## ADDED Requirements

### Requirement: Encrypted message framing
The system SHALL frame MTProto 2.0 encrypted messages as a 64-bit authorization-key
identifier, a 128-bit message key, and an AES-IGE-encrypted payload, and SHALL recompute
the message key from the decrypted plaintext and reject the message when it does not
match.

#### Scenario: Encrypted message round-trips
- **WHEN** a plaintext message is encrypted for an authorization key and then decrypted
- **THEN** the recovered plaintext equals the original and the integrity check passes

#### Scenario: Tampered ciphertext is rejected
- **WHEN** an encrypted payload is altered so the recomputed message key differs
- **THEN** decryption fails with an integrity error

#### Scenario: Plaintext is distinguished by a zero key identifier
- **WHEN** a frame carries a zero authorization-key identifier
- **THEN** it is treated as a plaintext message, not an encrypted one

### Requirement: Server salt
The system SHALL derive the initial server salt from the exchange nonces and SHALL carry
the current salt in encrypted messages, replacing it when the peer supplies a new salt.

#### Scenario: Initial salt matches the peer
- **GIVEN** an authorization key established by an exchange
- **WHEN** both sides derive the initial salt from the exchange nonces
- **THEN** they compute the same value

#### Scenario: Salt is replaced on request
- **WHEN** a peer supplies a new salt
- **THEN** subsequent messages use the new salt
