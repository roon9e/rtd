# Proposal

## Why

`rtd-error` can parse `FLOOD_WAIT`, but nothing retries it; there is no rate limiting or tracing
hook. `td` exposes an `Invoker` middleware chain with rate-limit, `FLOOD_WAIT` and tracing
decorators (`telegram/middleware.go`, `flood_wait.go`).

## What Changes

- An `Invoker` abstraction and a middleware chain applied per call.
- A `FLOOD_WAIT` retry middleware: on `FLOOD_WAIT_<n>`, wait `n` (via the injected clock) and
  retry, with a bounded attempt count.
- A simple rate-limit middleware and a tracing/metrics hook point.

## Capabilities

### New Capabilities

- `client-middleware`: invoker decorators for retry, rate limiting and observability.

## Impact

- **Crates**: `rtd-client` (new `middleware` module).
- **Non-goals**: a full OpenTelemetry integration (a server/observability change).
