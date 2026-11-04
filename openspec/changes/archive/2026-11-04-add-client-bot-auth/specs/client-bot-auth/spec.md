# Spec Delta

## Purpose

Defines bot-token authorization for the client.

## ADDED Requirements

### Requirement: Bot authorization

The client SHALL authorize a bot by calling `auth.importBotAuthorization` with its API id/hash
and the bot token, returning the typed `auth.Authorization`.

#### Scenario: Bot signs in
- **WHEN** the client submits a valid bot token
- **THEN** it returns `auth.Authorization` without a phone-code step

#### Scenario: Invalid token fails closed
- **WHEN** the token is invalid
- **THEN** the client returns a typed error and does not retry indefinitely
