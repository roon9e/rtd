# Spec Delta

## Purpose

Adds environment-based credential helpers.

## ADDED Requirements

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
