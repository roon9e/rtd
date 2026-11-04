# Proposal

## Why

The client has no datacenter awareness: no DC list, no resolver, no way to dial a media/CDN DC
or to follow a `_MIGRATE` redirect. `td` centralizes this in `telegram/dcs` (Plain/MTProxy/
Websocket/DNS resolvers) plus migration handling in `telegram/invoke.go`.

## What Changes

- A `Resolver` abstraction with default DC lists (production/test) and IPv6 preference.
- Resolver implementations: Plain (TCP), MTProxy (via `add-client-mtproxy-dialer`), WebSocket
  (via `add-client-websocket`), and DNS (encrypted config decode).
- Migration/redirect handling: `PHONE_MIGRATE_*`/`NETWORK_MIGRATE_*`/`FILE_MIGRATE_*`
  switch the target DC, and `dc_transfer` exports/imports authorization.

## Capabilities

### New Capabilities

- `client-dcs`: datacenter discovery, resolution and migration.

## Impact

- **Crates**: `rtd-client` (new `dcs` module); depends on `add-client-pool`,
  `add-client-mtproxy-dialer`, `add-client-websocket`.
- **Non-goals**: the server-side multi-DC runtime.
