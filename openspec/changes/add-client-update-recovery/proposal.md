# Proposal

## Why

`rtd-client` only dispatches `Updates`/`Update` objects as they arrive. It does not track
`pts`/`qts`/`seq`, detect gaps, or recover with `updates.getDifference`/`getChannelDifference`,
so it silently loses updates after a disconnect. `td`'s `telegram/updates` runs ordered,
gap-aware sequence boxes with channel state recovery.

## What Changes

- A `Manager` implementing the update handler with three sequence boxes (pts, qts, seq), a gap
  buffer, and short/too-long conversion.
- Difference recovery: `updates.getState`, `updates.getDifference` (Difference/Empty/Slice/
  TooLong) and per-channel `updates.getChannelDifference`.
- Channel state workers with a concurrency cap and access-hash stores.
- Callbacks `OnTooLong`, `OnChannelTooLong`, `OnChannelInaccessible`,
  `OnLoadUserStateFailed`/`OnLoadChannelStateFailed`, and a pluggable `StateStorage`.

## Capabilities

### New Capabilities

- `client-update-recovery`: ordered, gap-aware update processing and recovery.

## Impact

- **Crates**: `rtd-client` (new `updates` module); depends on `add-client-background-receive`.
- **Non-goals**: server-side update generation (an edge change).
