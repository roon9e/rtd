# Proposal

## Why

`LocalCore` keeps auth keys and layer evidence in memory, so a standalone `rtd-edge` (no remote
core) loses all state on restart. Multi-DC also needs a shared store. `openspec/config.yaml`
reserves storage traits and promises a first-party PostgreSQL backend at the multi-DC stage; this
turns that into a concrete change.

## What Changes

- Define durable storage traits for auth keys, sessions/layer evidence and delivery state.
- Provide an in-memory implementation (moved from `LocalCore`) plus a PostgreSQL-backed
  implementation, with Redis for ephemeral coordination.
- Wire `rtd-edge` to select a backend from config.

## Capabilities

### New Capabilities

- `server-storage`: durable server state behind storage traits.

## Impact

- **Crates**: `rtd-server` (storage traits + backends), `rtd-edge` (config/selection).
- **Non-goals**: cross-DC delivery semantics (a separate edge change uses this storage).
