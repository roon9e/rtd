# client-authorization Specification

## Purpose
Defines the client-side authorization calls: requesting a login code and signing in with it.

## Requirements

### Requirement: Login code and sign-in

The client SHALL provide typed calls for `auth.sendCode` and `auth.signIn` that return the
decoded `auth.SentCode` and `auth.Authorization` respectively, and SHALL fail closed when the
server answers with an unexpected constructor.

#### Scenario: The login flow returns typed results
- **WHEN** the client sends a login code and then signs in with it
- **THEN** `sendCode` returns an `auth.SentCode` and `signIn` returns an `auth.Authorization`

#### Scenario: An unexpected result fails closed
- **WHEN** the server answers `auth.sendCode` with a constructor other than `auth.SentCode`
- **THEN** the call returns an error rather than an unrelated value
