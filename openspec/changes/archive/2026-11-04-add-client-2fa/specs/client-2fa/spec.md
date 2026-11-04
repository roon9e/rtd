# Spec Delta

## Purpose

Defines two-factor (SRP) sign-in and password recovery for the client.

## ADDED Requirements

### Requirement: SRP sign-in

When the server answers `SESSION_PASSWORD_NEEDED`, the client SHALL fetch the password
parameters, compute the SRP proof, and call `auth.checkPassword`, returning the authorization.

#### Scenario: Password account signs in
- **WHEN** `signIn` answers `SESSION_PASSWORD_NEEDED` and a password is provided
- **THEN** the client completes `auth.checkPassword` and returns `auth.Authorization`

#### Scenario: Wrong password fails closed
- **WHEN** the server rejects the proof
- **THEN** the client returns a typed password error and does not retry indefinitely

### Requirement: Password recovery

The client SHALL provide typed helpers for recovery-code request, check and recovery.

#### Scenario: Recovery flow returns typed results
- **WHEN** the caller requests and checks a recovery code
- **THEN** the typed results are returned
