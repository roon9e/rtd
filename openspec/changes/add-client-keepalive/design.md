# Design

## Context

The client has one connection and no timers. `rtd-util` provides `Clock`, so ping scheduling
can be tested with `ManualClock`. The server already answers `ping`/`ping_delay_disconnect`.

## Goals / Non-Goals

**Goals**

- Detect dead connections and keep idle ones alive.
- Transparently retry unprocessed requests across a reconnect.

**Non-Goals**

- Connection pooling or multi-DC failover.

## Decisions

### D1. One ping loop driven by the injected clock

A task (async) or thread (sync) sends `ping_delay_disconnect` every `ping_interval` and expects
a `pong` within `ping_timeout`. A missed pong marks the connection dead. `td` uses ~60 s
interval and 5 s timeout; keep these configurable via `ClientOptions`.

### D2. Backoff reconnect reusing session data

On death, close the transport, wait `backoff.next()` (capped exponential), reconnect, and
re-handshake reusing `SessionData` (auth key, session id, salts). This mirrors
`Client::from_session`.

### D3. Retry unprocessed requests

Track in-flight calls; on reconnect, resend those neither answered nor acknowledged. Requests
never written are simply re-queued. This matches the `td` connection-loss recovery behavior
(#1030).

## Risks / Trade-offs

- [Retrying non-idempotent methods is unsafe] → retry only until the server acknowledges the
  message (`msgs_ack`); once acknowledged but unanswered, fail the call.
- [Timer jitter in tests] → all timing via `ManualClock`.
