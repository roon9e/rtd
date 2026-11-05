# Proposal

## Why

The parity audit left documented fidelity gaps below full package coverage: markup lacked
non-text inline buttons and bot menus; rich messages lacked most block kinds; media sends had no
schedule/reply options and no story or paid-media senders; and `oteltg` recorded spans but no
metrics.

## What Changes

- Markup: more inline button kinds and bot-menu buttons; reply-keyboard `requestPeer`.
- Rich messages: list, preformatted, quote, divider, details, footer/header/subheader/subtitle.
- Media sends: `MediaOptions` for reply/schedule/silent/background/noforwards/invert, plus story
  and paid-media senders.
- Observability: an OpenTelemetry metrics recorder alongside the tracer.

## Capabilities

### Modified Capabilities

- `client-markup`, `client-rich-messages`, `client-media-senders`, `client-observability`.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: `tdp` byte/date rendering (needs codegen field metadata) and invoice/todo senders.
