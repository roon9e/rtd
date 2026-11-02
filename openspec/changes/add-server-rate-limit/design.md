# Design

## Context

Dispatch already binds a layer and routes to a handler. The actor records connection/session
identity (auth key, session id, IP). gramsrv has `sensitive_rate_limit` and emits
`FLOOD_WAIT`.

## Goals / Non-Goals

**Goals**

- Deterministic, clock-driven limits with `FLOOD_WAIT` responses.
- Sensible presets for sensitive methods.

**Non-Goals**

- Global/distributed counters.

## Decisions

### D1. Token buckets keyed by `(method-class, identity)`

A `RateLimiter` holds token buckets per method class and identity (auth key id or source IP for
pre-auth). The injected `Clock` drives refill.

### D2. Method classes, not per-method tables

Classify methods (login, resolve, messaging, file, other) with per-class limits, keeping config
small while covering the sensitive paths.

### D3. `FLOOD_WAIT` is a normal RPC error

On refusal, return `rpc_error 420 FLOOD_WAIT_<seconds>` through the existing error path, so
clients respect it.

## Risks / Trade-offs

- [Blocking legitimate bursts] → generous defaults and per-class tuning; no limiting of service
  messages.
