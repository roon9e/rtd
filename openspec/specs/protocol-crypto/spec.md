# Protocol Crypto Specification

## Purpose

Defines the cryptographic behavior MTProto depends on: how messages are encrypted and
integrity-checked, how auth keys are derived and identified, how the auth-key exchange
is protected, and how two-factor and Diffie-Hellman material is validated.

## Requirements

### Requirement: Message encryption and integrity
The system SHALL encrypt and decrypt MTProto messages with AES-IGE using padding that
is a multiple of 16 bytes, at least 12 bytes, and extended by a random number of extra
blocks. It SHALL recompute the message key from the plaintext after decryption and
reject the message if it does not match.

#### Scenario: Tampered message is rejected
- **WHEN** a decrypted message's recomputed message key differs from the received one
- **THEN** decryption fails with an integrity error

#### Scenario: Padding defeats length fingerprinting
- **WHEN** two equal-length plaintexts are encrypted
- **THEN** their ciphertext lengths may differ because of random extra padding blocks

### Requirement: Message key derivation
The system SHALL derive message keys and AES key/IV from the auth key using the
MTProto 2.0 key-derivation function with a side-dependent offset, and SHALL support the
legacy key-derivation function required for binding temporary authorization keys.

#### Scenario: Client and server derive matching keys
- **GIVEN** the same auth key and message key
- **WHEN** the client side and the server side each derive AES key and IV
- **THEN** each side's derived values allow it to decrypt what the other encrypts

### Requirement: Authorization key identity
The system SHALL derive a stable 64-bit auth-key identifier from the auth key and SHALL
expose it so messages can be routed to the correct stored key.

#### Scenario: Identifier is stable
- **WHEN** the identifier is derived twice from the same auth key
- **THEN** both results are equal

### Requirement: Auth-key exchange protection
The system SHALL protect the auth-key exchange using the MTProto RSA padding scheme
that reverses and hashes the inner data before RSA encryption, and SHALL support the
inverse operation for receiving an exchange request.

#### Scenario: Exchange payload round-trips
- **WHEN** an inner exchange payload is protected for a public key and then unwrapped with the matching private key
- **THEN** the recovered inner payload equals the original

### Requirement: Diffie-Hellman and factorization validation
The system SHALL validate Diffie-Hellman parameters, including range checks and
primality of both the prime and half the prime-minus-one, and SHALL factorize the
client's composite number only when it is within the accepted bounds.

#### Scenario: Weak prime is rejected
- **WHEN** a prime that fails the primality or range checks is supplied
- **THEN** the exchange fails with a validation error

### Requirement: Temporary exchange keys and nonces
The system SHALL derive temporary AES keys, nonce hashes and the initial server salt
from exchange nonces at fixed widths so that leading zero bytes are preserved.

#### Scenario: Leading-zero nonce is preserved
- **WHEN** an exchange nonce begins with a zero byte
- **THEN** the derived temporary keys and salt match those derived by a peer using the same fixed-width rules

### Requirement: Secure random source
All security-relevant randomness SHALL come from a cryptographically secure random
source, and key material SHALL NOT be written to logs or formatted output.

#### Scenario: Key material is redacted
- **WHEN** an auth key is formatted for logging or diagnostics
- **THEN** its secret bytes are not included

### Requirement: Two-factor authentication hashing
The system SHALL implement the SRP-based password hashing and proof computation used
by two-factor authentication, including creating a new password hash from a fresh salt.

#### Scenario: Password proof verifies
- **WHEN** a client computes its proof from the server's SRP parameters and the server validates it
- **THEN** the proof is accepted for the correct password and rejected otherwise
