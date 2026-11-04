# Proposal

## Why

`td`'s `telegram/message/markup` builds every reply markup: inline keyboards but also reply
keyboards, keyboard hiding and force-reply. `rtd-client::markup` only builds inline keyboards,
so callers cannot request a phone number, a location, a poll or a web view from a keyboard.

## What Changes

- Add reply-keyboard builders: `ReplyButton`/`ReplyButtonKind`, `ReplyKeyboardOptions`,
  `reply_keyboard`, `reply_keyboard_hide` and `reply_keyboard_force_reply`.
- Keep the existing `InlineButton`/`inline_keyboard`.

## Capabilities

### New Capabilities

- `client-markup`: inline and reply keyboard builders.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: bot-menu button styling and `RequestPeer` buttons.
