# Design

## Context

Peers need an `access_hash`; resolving a username/id goes through `contacts.resolveUsername` or
`users.getUsers`/`messages.getChats`. `td` caches entities, feeds hashes from updates and offers
convenience wrappers.

## Goals / Non-Goals

**Goals**

- Resolve and cache users/chats/channels with access hashes.
- Keep them fresh from updates.

**Non-Goals**

- Full admin-log/boosts feature set (later).

## Decisions

### D1. Entities wrap TL objects plus an access hash

`User`/`Chat`/`Channel` hold the TL object and its `input_peer`; `Sync` re-fetches from the
server.

### D2. Storage is a trait

`Storage` maps `PeerId -> Entity`; the default is in-memory. Update application replaces cached
entities when a matching `Update*` arrives.

### D3. Members is a submodule

Channel/chat member iteration and rights helpers live in `peers::members`, using the query
iterators.

## Risks / Trade-offs

- [`peers` is experimental in `td`] → keep the surface small and stable.
