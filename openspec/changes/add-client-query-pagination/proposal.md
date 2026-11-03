# Proposal

## Why

Paging through participants, dialogs, messages, photos or blocked users requires offset/hash
bookkeeping. `rtd-client` has none. `td` ships `telegram/query` with generated iterators and a
`cached` subpackage for hash-based caching.

## What Changes

- Generic `Query` iterators for participants, blocked, photos, dialogs, messages, featured
  stickers and cached queries.
- Offset/hash management with `ForEach`/`Fetch` and a request hasher.

## Capabilities

### New Capabilities

- `client-query-pagination`: paginated query iterators.

## Impact

- **Crates**: `rtd-client` (new `query` module).
- **Non-goals**: peers resolution (separate change).
