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
