# Design

## Context

`SessionRegistry` tracks `(auth_key_id, session_id)` with idle eviction and an `AuthKeyCache`.
On shutdown the edge currently stops accepting but does not drain in-flight dispatches.

## Goals / Non-Goals

**Goals**

- A session states machine safe across reconnect and revocation.
- Deterministic shutdown that drains work.

**Non-Goals**

- A distributed session store (that is `add-server-storage`).

## Decisions

### D1. Explicit lifecycle states

`SessionState { Provisional, Ready, Revoked }`; a provisional key not confirmed within a deadline
is revoked. `new_session_created` promotes to ready.

### D2. Shutdown gate with a barrier

A `ShutdownGate` stops new admissions and a barrier waits for in-flight dispatches to complete
(or times out), mirroring gramsrv's `shutdown_gate`.

### D3. Quick-ack and pending flush

Unacknowledged outbound messages are flushed or retried before eviction, and a quick-ack deadline
bounds how long a result waits for a client ack (feeding `add-edge-delivery-fabric`).

## Risks / Trade-offs

- [Barrier hang on a stuck call] → bound the drain with a timeout and log stragglers.
