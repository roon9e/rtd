# Design

## Context

`ClientSession` wraps an `rtd_crypto::AuthKey` (a 2048-bit `Key` plus its 64-bit fingerprint)
and an `rtd_proto::Session` (session id, salt, allocation state). The authorization key is the
secret that makes the exchange worth avoiding on reconnect; the session id and salt are the
minimal additional state a server accepts for a reconnected session.

## Goals / Non-Goals

**Goals**

- A small, versioned, dependency-free snapshot of the durable session state.
- Rebuild a `ClientSession` from it, deriving the key fingerprint rather than trusting it.

**Non-Goals**

- A storage backend, key rotation, or PFS temporary-key binding.

## Decisions

### D1. A versioned little-endian blob, not a serde format

The workspace keeps serialization dependency-free and TL-oriented, so `SessionData` encodes a
fixed layout: the four-byte magic `RTDS`, a little-endian version, then the fixed-width fields
(the 256-byte key, the session id and the salt). Decoding validates the magic, version and
length and fails closed, so a truncated or foreign blob is rejected rather than misread.

### D2. The snapshot holds only session state

The datacenter is not included: the caller knows it and must open the transport for that dc
before calling `Client::from_session`. Keeping the snapshot to key/id/salt makes it usable
regardless of how the connection is established (plain, obfuscated, FakeTLS).

### D3. The key fingerprint is derived, not stored

`from_data` rebuilds the `AuthKey` with `Key::with_id()`, recomputing `auth_key_id` from the
key, so a snapshot cannot carry a key that disagrees with its fingerprint.

## Risks / Trade-offs

- [The blob is secret and unsigned] → the caller stores it confidentially; a tampered blob
  simply fails the next exchange/decrypt and the caller re-authenticates.
- [A format change breaks old blobs] → the version field lets `decode` reject unknown versions.
