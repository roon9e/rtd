# Proposal

## Why

`rtd-client` persists a single versioned `RTDS` blob and has no storage abstraction, so it
cannot use a file backend, a database, or read `td`/Telethon/Telegram-Desktop sessions. `td`
ships a `Storage` interface with File/Memory/JS/Telethon/TDesktop backends.

## What Changes

- A `Storage` trait (`load`/`store`/`delete`) for persisted session data.
- Backends: file (mode 0600), in-memory, and readers for Telethon string sessions and Telegram
  Desktop `key_datas`/`tdata`.
- A loader that handles the versioned blob and a `Config` snapshot.

## Capabilities

### New Capabilities

- `client-session-backends`: pluggable session persistence backends.

## Impact

- **Crates**: `rtd-client` (new `session` backend module).
- **Non-goals**: server-side auth-key storage (a server/edge change).
