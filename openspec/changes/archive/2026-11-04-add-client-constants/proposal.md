# Proposal

## Why

`td` ships `constant`, a small package of Telegram-defined IDs, upload limits,
test credentials, and the `TDLibPeerID` type that packs users, chats and channels
into one `i64` namespace. `rtd` has no equivalent, so callers cannot classify or
convert peer ids the way TDLib/Bot-API users expect.

## What Changes

- Add a `constant` module to `rtd-client` with the well-known user IDs, upload
  limits and test credentials.
- Add `TdlibPeerId` with user/chat/channel constructors, `to_plain`, and
  `is_user`/`is_chat`/`is_channel`/`is_monoforum` predicates.

## Capabilities

### New Capabilities

- `client-constants`: Telegram constants and TDLib peer-id encoding.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: none; the module is pure data.
