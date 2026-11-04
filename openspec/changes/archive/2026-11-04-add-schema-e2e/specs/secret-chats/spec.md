# Spec Delta

## Purpose

Defines a generated end-to-end (secret chat) value model.

## ADDED Requirements

### Requirement: Secret-chat schema is generated

The generator SHALL import `_schema/encrypted.tl` and emit committed Rust codecs for its types,
including `DecryptedMessage`, its media and actions, and `DocumentAttribute`.

#### Scenario: A decrypted message round-trips
- **WHEN** a `DecryptedMessage` is encoded and decoded
- **THEN** the decoded value equals the original

#### Scenario: Generation is fresh
- **WHEN** the generator is re-run
- **THEN** it produces no diff
