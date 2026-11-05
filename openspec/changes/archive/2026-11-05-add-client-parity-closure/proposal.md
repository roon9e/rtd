# Proposal

## Why

The parity audit's remaining residuals were the query long tail, the invoice/todo senders, and
environment-based credential helpers.

## What Changes

- Query: a `messages.search` iterator (`SearchIter`/`fetch_search`).
- Media sends: invoice (`send_invoice`) and to-do (`send_todo`) senders.
- Configuration: environment credential helpers (`api_credentials_from_env`).

## Capabilities

### Modified Capabilities

- `client-query-extras`, `client-media-senders`, `client-ergonomics`.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: `tdp` byte/date rendering (needs generated per-field accessors).
