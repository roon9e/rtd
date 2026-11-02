# Proposal

## Why

`rtd-client` establishes an encrypted session by completing the authorization-key exchange,
which is expensive (a Diffie-Hellman handshake). A long-lived client must be able to reconnect
without repeating it. The session's durable state — the authorization key, the session
identifier and the current server salt — is not yet serializable, so it cannot be saved and
restored.

## What Changes

- A `SessionData` snapshot in `rtd-client`: the raw 2048-bit authorization key, the session
  identifier and the server salt, with a stable, versioned byte encoding.
- `ClientSession::snapshot`/`from_data` and `Client::session_data`/`from_session` so a caller
  can persist a session and rebuild a client over a fresh transport without a new exchange.

## Capabilities

### New Capabilities

- `client-session-persistence`: save and restore a client session's authorization key,
  session identifier and salt.

## Impact

- **Crates**: `rtd-client` only; no existing behavior changes.
- **Non-goals**: no storage backend (the caller writes the bytes), no key rotation, and no
  binding of temporary (PFS) keys.
- **Security**: the encoding contains the secret key; it carries no signature or encryption,
  so the caller must store it confidentially.
