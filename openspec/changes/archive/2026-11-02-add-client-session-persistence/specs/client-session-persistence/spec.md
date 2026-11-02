# Spec Delta

## Purpose

Defines how an established MTProto client session is saved and restored so a reconnect does
not repeat the authorization-key exchange.

## ADDED Requirements

### Requirement: Session snapshot

The client SHALL serialize an established session's durable state — the authorization key, the
session identifier and the server salt — into a versioned blob, and SHALL rebuild a session
from it with the key fingerprint derived from the key.

#### Scenario: A snapshot round trips
- **WHEN** a session is snapshotted and the blob is decoded
- **THEN** the authorization key, session identifier and salt are unchanged

#### Scenario: A malformed snapshot fails closed
- **WHEN** a blob has a wrong magic, an unknown version, or a truncated body
- **THEN** decoding returns an error rather than a partial session

### Requirement: Restore without re-exchange

A client SHALL be constructible from a snapshot over a fresh transport, reuse the saved
authorization key, session identifier and salt, and invoke without a new authorization-key
exchange.

#### Scenario: A restored client reuses the session
- **WHEN** a client is built from a snapshot
- **THEN** it reports the saved session identifier, salt and authorization key
