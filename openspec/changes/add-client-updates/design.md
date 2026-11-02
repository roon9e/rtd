# Design

## Context

The synchronous `Client` reads from the socket only inside `await_result`, so any server
message that arrives between calls is not read at all, and one that arrives during a call is
inspected only for an `rpc_result`. The canonical model already carries the update envelope
(`rtd_tg::Updates`) and its members (`rtd_tg::Update`), so dispatch is a decode-and-call.

## Goals / Non-Goals

**Goals**

- Deliver server updates to a caller handler with a minimal, synchronous mechanism.
- Preserve the request/response path and the fail-closed discipline.

**Non-Goals**

- A background reader (updates arrive only while a call is blocked on a reply), update
  gap/state recovery, and an async handler.

## Decisions

### D1. A handler invoked from the receive loop

`Client` holds an optional `Box<dyn FnMut(Updates) + Send>`. In the receive loop, an inbound
message that is not an `rpc_result`, container, gzip or bad-salt is offered to the handler:
`Updates::decode` first, then `Update::decode`, wrapping a bare `Update` in
`Updates::updateShort`. An unrecognised body is ignored, as before. The `Send` bound lets the
handler live in the async façade's actor thread.

### D2. In-band delivery is the first step

Because the client is request/response and synchronous, updates are observed only while a call
awaits its reply. A background receive loop (a reader thread feeding a queue) is a follow-up;
this change makes the common case — updates that accompany a reply — observable, which is what
the tests exercise.

## Risks / Trade-offs

- [Updates between calls are missed] → explicitly a non-goal here; the follow-up reader loop
  addresses it.
- [A slow handler stalls the call] → acceptable while the actor is single-threaded; a queue
  decouples it later.
