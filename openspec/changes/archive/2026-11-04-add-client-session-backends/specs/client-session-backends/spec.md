# Spec Delta

## Purpose

Defines pluggable session persistence backends for the client.

## ADDED Requirements

### Requirement: Storage abstraction

The client SHALL persist session data through a `Storage` abstraction and SHALL preserve the
existing versioned on-disk format.

#### Scenario: File backend round-trips
- **WHEN** a session is stored and reloaded from a file backend
- **THEN** the auth key and session state are restored

### Requirement: Foreign session import

The client SHALL import Telethon and Telegram Desktop sessions into its own session data.

#### Scenario: Telegram Desktop session is imported
- **WHEN** a `tdata` directory and password are supplied
- **THEN** the client produces a `SessionData` with the account's auth key
