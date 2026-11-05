# client-ergonomics Specification

## Purpose
Defines client ergonomics: current-user lookup, raw-JSON invoke and ASCII helpers.

## Requirements

### Requirement: Current user

The client SHALL return the authorized account's user.

#### Scenario: Self user is returned
- **WHEN** the client asks for the current user
- **THEN** the self user is returned

### Requirement: Raw JSON invoke

The client SHALL accept a JSON method request as text, invoke it, and return the JSON response.

#### Scenario: JSON text is invoked
- **WHEN** a JSON method request string is invoked
- **THEN** a JSON response string is returned

### Requirement: ASCII helpers

The client SHALL provide ASCII classification helpers for digits, letters and case.

#### Scenario: ASCII digit is recognized
- **WHEN** an ASCII digit is classified
- **THEN** the digit predicate is true and the letter predicate is false

### Requirement: Environment credentials

The client SHALL read application credentials from the environment, accepting
`TELEGRAM_API_ID`/`TELEGRAM_API_HASH` or `API_ID`/`API_HASH`, and SHALL reject incomplete or
non-numeric values.

#### Scenario: Credentials are read
- **WHEN** a valid id and hash pair is present in the environment
- **THEN** the credentials are returned

#### Scenario: Incomplete credentials are rejected
- **WHEN** the id is missing or the hash is empty
- **THEN** no credentials are returned
