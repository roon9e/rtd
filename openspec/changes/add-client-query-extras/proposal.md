# Proposal

## Why

`td`'s `telegram/query` also has featured-sticker and cached query helpers. `rtd-client`'s
pagination covers history, dialogs, participants, photos and blocked users only.

## What Changes

- Add a featured-stickers iterator.
- Add cached query helpers backed by a hash-aware cache.

## Capabilities

### New Capabilities

- `client-query-extras`: featured-sticker and cached query helpers.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: the full set of `td` query subpackages.
