# Proposal

## Why

Working with users, chats and channels requires resolving peers, caching their access hashes and
applying updates to keep them fresh. `rtd-client` has none of this. `td` ships
`telegram/peers` (with `members`) — a `Manager` over `User`/`Chat`/`Channel` entities.

## What Changes

- A `Manager` resolving peers (`ResolveUserID`/`ResolvePeer`/`FromInputPeer`) and caching
  them, with a pluggable storage (in-memory default).
- Entity types (`User`, `Chat`, `Channel`, `Supergroup`) with `InputPeer`, `VisibleName`,
  `Username`, `Sync`, `Report`, `Photo`.
- Update application and channel member management (`members`).

## Capabilities

### New Capabilities

- `client-peers`: peer resolving, caching and member management.

## Impact

- **Crates**: `rtd-client` (new `peers` module); depends on `add-client-query-pagination`.
- **Non-goals**: message builders (separate change).
