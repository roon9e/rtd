# client-qr-login Specification

## Purpose
Defines QR-token login for the client.

## Requirements

### Requirement: Export and import

The client SHALL export a login token and, after it is accepted, import it to obtain
`auth.Authorization`.

#### Scenario: Exported token is imported
- **WHEN** a token is exported, accepted by another client, and imported
- **THEN** the client returns `auth.Authorization`

### Requirement: Token callback and expiry

The client SHALL surface each exported token with its expiry and SHALL stop when the token
expires.

#### Scenario: Token expires
- **WHEN** the token expiry passes without import
- **THEN** the driver returns a typed expiry error
