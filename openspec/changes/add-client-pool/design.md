# Design

## Context

`rtd_client::Client<T>` is one transport. `td`'s `pool.DC` keeps a set of `Conn`s, hands them
out, and rebuilds them on failure. The sans-IO core already makes a connection reusable via
`SessionData`.

## Goals / Non-Goals

**Goals**

- Multiple connections per DC with safe acquire/release and dead-conn replacement.
- Transparent retry of a request that failed on a dead connection.

**Non-Goals**

- CDN pools, DC discovery, or migration.

## Decisions

### D1. Handles with RAII release

`Pool::acquire()` returns a `PoolConn` that releases on drop; the pool tracks free and in-use
connections and closes idle ones past a cap.

### D2. Failure is detected on acquisition and on invoke

A connection marked dead by the keepalive loop (`add-client-keepalive`) is dropped from the
free list; an invoke error that is a connection error triggers replacement and one retry,
mirroring `td`'s `pool.DC.Invoke` re-acquire.

### D3. PFS-aware key selection

`SessionOptions` selects the permanent or temporary auth key per connection, consistent with
`rtd-exchange`'s temp mode.

## Risks / Trade-offs

- [Double-send on retry] → only retry when the failure is a connection error before
  acknowledgement, matching `add-client-keepalive`'s rule.
