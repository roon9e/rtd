# Proposal

## Why

`rtd-client` is synchronous: its core, the authorization-key exchange and its transport all
use blocking I/O. The project's frozen runtime decision is tokio, and the server crates are
async, so an async caller cannot use the client without blocking a runtime worker.

## What Changes

- An async client façade (`AsyncClient`) that owns a blocking actor thread running the
  existing synchronous `Client` and exposes `async` `initialize`/`invoke` over a command
  channel, mirroring how `rtd-server` bridges its synchronous exchange into its async actor.
- `AsyncClient::spawn` (over any `Transport`) and `AsyncClient::connect_tcp` (dial then
  spawn), with shutdown on drop.

## Capabilities

### New Capabilities

- `client-async`: an asynchronous façade over the synchronous client core for tokio callers.

## Impact

- **Crates**: `rtd-client` gains a `tokio` dependency (`sync`); no existing behavior changes.
- **Non-goals**: a fully async transport/exchange (the actor thread keeps the blocking core),
  and DC migration.
- **Trade-off**: one thread per client and a command hop per call; the async API does not
  block the runtime, which is the point.
