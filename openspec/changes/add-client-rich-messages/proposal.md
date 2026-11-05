# Proposal

## Why

`td`'s `telegram/message/rich` builds Telegram rich messages (layer 227+) as trees of
`PageBlock`/`RichText`. `rtd-client::message` always sets `rich_message: None`, so callers cannot
send or edit rich content.

## What Changes

- Add builders for the common `RichText` and `PageBlock` nodes.
- Parse the rich HTML/Markdown subset and plumb `rich_message` through send/edit.

## Capabilities

### New Capabilities

- `client-rich-messages`: rich-message construction.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: every page-block kind; start with headings, paragraphs, lists, quotes, pre and
  text formatting.
