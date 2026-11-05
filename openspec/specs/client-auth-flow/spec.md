# client-auth-flow Specification

## Purpose
Defines the high-level login flow and locked 2FA password handling.

## Requirements

### Requirement: Login flow

The client SHALL expose a login flow that reports the current authorization status and, when
authorization is needed, drives sending/resending the code, signing in, completing sign-up and
accepting the terms of service, returning the authorized session.

#### Scenario: Authorized session is reused
- **WHEN** the stored session is already authorized
- **THEN** the flow reports success without sending a code

#### Scenario: Sign-in completes
- **WHEN** the flow sends a code and signs in with it
- **THEN** the flow returns an authorized session

### Requirement: Held password

The client SHALL accept a two-factor password held in a buffer that is zeroized when dropped,
and SHALL not expose the password through `Debug`.

#### Scenario: Password is redacted in Debug output
- **WHEN** the held password is formatted with `Debug`
- **THEN** the output is redacted

#### Scenario: Password is cleared on drop
- **WHEN** the held password buffer is dropped
- **THEN** its backing bytes are zeroized
