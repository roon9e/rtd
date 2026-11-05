# Proposal

## Why

`td`'s `telegram/message` also stages stickers, typing, drafts, reports and join requests, and
supports scheduling and replies. `rtd-client::message` has no schedule/reply options and lacks
those senders.

## What Changes

- Add `reply_to` and `schedule_date` options to `RequestBuilder` and the media senders.
- Add sticker senders, `set_typing`, draft save, report and join helpers.

## Capabilities

### New Capabilities

- `client-message-staging`: scheduling, replies, typing, drafts, reports, joins and stickers.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: scheduled-message management beyond sending.
