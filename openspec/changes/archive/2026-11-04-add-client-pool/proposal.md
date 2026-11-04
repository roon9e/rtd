# Proposal

## Why

`rtd-client` owns exactly one connection, so it cannot spread load, survive a single dead
connection without a full reconnect, or talk to media-only/backup connections. `td` pools
connections per DC (`pool.DC`) with acquire/release, dead-connection replacement and request
retry.

## What Changes

- A per-DC connection pool: acquire/release, a free list, `MaxOpenConnections`, and replacement
  of dead connections.
- A `Session`/`SessionOptions` carrying DC, auth key and salt, with PFS-aware key selection.
- Request routing that picks a connection, retries on a dead one, and balances across the pool.

## Capabilities

### New Capabilities

- `client-pool`: per-datacenter connection pooling for the client.

## Impact

- **Crates**: `rtd-client` (new `pool` module); depends on `add-client-background-receive`.
- **Non-goals**: CDN pools and DC discovery (`add-client-dcs`).
