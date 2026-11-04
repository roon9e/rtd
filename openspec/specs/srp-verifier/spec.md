# srp-verifier Specification

## Purpose
Defines server-side SRP-6a verification for Telegram 2FA.

## Requirements

### Requirement: Verifier derivation

The server SHALL derive the SRP verifier from a password and the server's `PasswordKdfAlgo`,
using the same derivation as the client proof.

#### Scenario: The client half verifies
- **WHEN** the client computes its proof with `hash` for a password and the server verifies it
- **THEN** verification succeeds

### Requirement: Proof verification

The server SHALL derive `B` from the verifier and a random `b`, SHALL reject `B ≡ 0 (mod p)`,
and SHALL verify the client proof `M1` in constant time, producing `M2`.

#### Scenario: Wrong password fails
- **WHEN** the client proves with the wrong password
- **THEN** `M1` verification fails
