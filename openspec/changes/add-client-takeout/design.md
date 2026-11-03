# Design

## Context

`account.initTakeoutSession` returns a takeout id; subsequent calls are wrapped in
`invokeWithTakeout { takeout_id, query }`; `account.finishTakeoutSession` ends it. `td` exposes
a `Client` whose `Invoke` wraps automatically.

## Goals / Non-Goals

**Goals**

- A scoped wrapper that guarantees `finishTakeoutSession`.
- A typed `Config`.

**Non-Goals**

- Archive assembly; rate-limit tuning.

## Decisions

### D1. RAII-style session

`run` initializes, invokes `f` with a `TakeoutInvoker`, and finishes even on error, matching
`td`. The invoker wraps each call in `invokeWithTakeout`.

### D2. Config maps to `initTakeoutSession` flags

`Config { contacts, message_users, message_chats, message_megagroups, message_channels, files,
file_max_size }` maps directly to the flags.

## Risks / Trade-offs

- [Interrupt leaves a session open] → best-effort finish in a drop guard; document the residual.
