# Design

## Context

List methods (`channels.getParticipants`, `messages.getDialogs`, `messages.getHistory`,
`photos.getUserPhotos`, …) take `offset`/`limit` and return a slice plus count. `td` generates an
iterator per method and a `cached` variant keyed by a request hash.

## Goals / Non-Goals

**Goals**

- A uniform iterator trait with `next`/`ForEach`/`Fetch`.
- Correct termination (short page or empty).

**Non-Goals**

- A generic reflection-based query generator (hand-write per method).

## Decisions

### D1. `Query<T>` with `next(api) -> Option<Vec<T>>`

Each method implements a `next` that advances the offset and returns the page; the generic
driver loops until a short or empty page. `ForEach` calls a closure per element.

### D2. `cached` uses `messages.getMessages` hashing

Cached queries fetch fresh hashes first and only request changed items, matching
`telegram/query/cached`.

## Risks / Trade-offs

- [Per-method boilerplate] → acceptable; generation is a later optimization.
