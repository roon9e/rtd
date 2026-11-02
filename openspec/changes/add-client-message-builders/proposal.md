# Proposal

## Why

Sending anything beyond a raw `messages.sendMessage` requires builders for text, entities,
media, keyboards and inline results. `rtd-client` has only raw `invoke`. `td` ships
`telegram/message` with entity/html/markdown/rich/styling/markup/inline/unpack subpackages.

## What Changes

- A `Sender`/`RequestBuilder` with options (silent, background, clear, reply, schedule, no
  webpage, markup, send-as) and media/content senders (audio, document, photo, video, gif,
  sticker, poll, geo, contact, media group, …).
- Text and entity builders, HTML/Markdown parsing, and a rich-message builder.
- Inline keyboard/inline-result builders and an unpack helper for send results.

## Capabilities

### New Capabilities

- `client-message-builders`: typed message, media and markup construction and sending.

## Impact

- **Crates**: `rtd-client` (new `message` module); depends on `add-client-uploader`.
- **Non-goals**: rich-message rendering by clients (callers render).
