# Design

## Context

`messages.sendMessage`/`sendMedia` take `InputPeer`, a `message`, a `random_id`, a reply and
optional `reply_markup`. `td` layers builders over these and a peer resolver over
`contacts.resolveUsername`/`users.getUsers`.

## Goals / Non-Goals

**Goals**

- Ergonomic builders that produce the correct TL objects.
- Formatted text with entities (HTML/Markdown).

**Non-Goals**

- Rich rendering; takeout; calls.

## Decisions

### D1. Builder produces TL, sender invokes

`RequestBuilder` accumulates options and produces a `tg::Method`; `Sender::send` invokes it via
the `Invoker`. This keeps builders pure and testable.

### D2. Entity parsing mirrors `td`

`entity::Builder` parses HTML/Markdown into `MessageEntity` objects with a stable option set
(spoiler support included), matching `telegram/message/{html,markdown}`.

### D3. Peer resolution is separate

A `message::peer` resolver maps usernames/ids to `InputPeer` using `add-client-peers`; builders
accept an already-resolved peer.

## Risks / Trade-offs

- [Large surface] → implement content senders incrementally, prioritizing text/media/keyboards.
