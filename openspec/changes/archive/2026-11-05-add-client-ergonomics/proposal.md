# Proposal

## Why

`td`'s client root has `Self`, `InvokeRawJSON` and environment-based construction, and the
`ascii` package. `rtd-client` lacks these conveniences.

## What Changes

- Add `Client::self_user` (needs bare-vector results resolved).
- Add JSON text parsing/serialization and `Client::invoke_raw_json`.
- Add ASCII rune predicates.

## Capabilities

### New Capabilities

- `client-ergonomics`: current-user lookup, raw-JSON invoke and ASCII helpers.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: reading credentials from process environment by default.
