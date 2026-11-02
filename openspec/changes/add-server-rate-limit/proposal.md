# Proposal

## Why

A public edge has only static connection/handshake/frame budgets; there is no per-method rate
limiting or `FLOOD_WAIT` emission. `td`/gramsrv enforce sensitive-method limits and answer
`FLOOD_WAIT_<n>`. Without this, a client can hammer login or file methods.

## What Changes

- A per-method (and per-identity) rate limiter in `rtd-server`, applied before dispatch.
- `FLOOD_WAIT_<n>` errors when a limit is exceeded, matching the Telegram contract.
- Sensitive-method presets (login, resolve, file) with configurable windows.

## Capabilities

### New Capabilities

- `server-rate-limit`: per-method rate limiting and `FLOOD_WAIT` emission.

## Impact

- **Crates**: `rtd-server` (`rpc.rs` dispatch, new `ratelimit` module).
- **Non-goals**: distributed limiting across edges (the central store change can add it).
