# Design

## Context

Calls go through `Client::invoke`. `rtd_error::as_flood_wait` returns the wait duration. The
injected `Clock` makes waiting testable without real sleep.

## Goals / Non-Goals

**Goals**

- Compose decorators around invoke without changing call sites.
- Deterministic `FLOOD_WAIT` tests.

**Non-Goals**

- Distributed rate limiting or metrics export.

## Decisions

### D1. `Invoker` mirrors `td`

```
pub trait Invoker { fn invoke(&self, method: Method) -> Result<Object>; }
```

`Client` implements it; middleware wraps another `Invoker`. Order is outermost-first, as in
`td`.

### D2. FLOOD_WAIT retry uses the injected clock

`FloodWait::invoke` sleeps `as_flood_wait(err)` via `Clock` and retries up to `max_retries`,
then surfaces the error. No real `sleep` in tests.

### D3. A no-op tracing hook

A `TracingMiddleware` trait with a default no-op, so an OpenTelemetry adapter can be added
without touching the client.

## Risks / Trade-offs

- [Retrying a flooded mutating call] → retry only when the server returned `FLOOD_WAIT` (the
  request was not executed).
