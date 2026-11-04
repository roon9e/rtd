# client-middleware Specification

## Purpose
Defines invoker middleware for retry, rate limiting and observability.

## Requirements

### Requirement: Middleware chain

The client SHALL allow composing `Invoker` decorators around every call, applied outermost
first.

#### Scenario: A middleware observes a call
- **WHEN** a call is made through a configured middleware chain
- **THEN** each middleware sees the request before the next and the response on the way back

### Requirement: FLOOD_WAIT retry

A `FLOOD_WAIT` middleware SHALL wait the indicated duration via the injected clock and retry,
bounded by a maximum attempt count.

#### Scenario: Flood wait is retried
- **WHEN** the server answers `FLOOD_WAIT_3`
- **THEN** the middleware waits three seconds and retries, surfacing the error only after the
  attempts are exhausted
