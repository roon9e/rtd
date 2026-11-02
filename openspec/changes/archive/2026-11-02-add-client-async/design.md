# Design

## Context

The authorization-key exchange (`rtd_exchange::client_exchange`) and the transport
(`rtd_exchange::Stream`) are synchronous by design, and `rtd-server` bridges them into its
async actor with channels (`actor.rs::ChannelStream` + `spawn_blocking`). `rtd-client`'s
`Client` is likewise synchronous. A fully async transport would require reimplementing the
exchange and the codec over async I/O; that is not needed to make the client usable from
tokio.

## Goals / Non-Goals

**Goals**

- An `async` `initialize`/`invoke` usable from a tokio task without blocking the runtime.
- Reuse of the synchronous `Client` unchanged.

**Non-Goals**

- A fully async transport, exchange or RPC loop; multiple concurrent in-flight requests.

## Decisions

### D1. A blocking actor thread behind a command channel

`AsyncClient::spawn` starts a `std::thread` that owns the synchronous `Client` (dialing and
exchanging first, when created from an address) and serves a `tokio::sync::mpsc` of
`Initialize`/`Invoke` commands, each with a `oneshot` reply. The async methods send a command
and await the reply, so the runtime worker never blocks. This is the same "sans-IO core in a
blocking actor" shape `rtd-server` already uses for the exchange.

### D2. Startup errors surface synchronously

`spawn` performs the exchange on the actor thread and reports the outcome over a
`std::sync::mpsc` handshake, so `spawn` returns `Err` (and joins the thread) when the
connection or exchange fails, rather than deferring it to the first call.

### D3. Shutdown on drop

Dropping `AsyncClient` closes the command channel (and sends a `Shutdown` best-effort), which
ends the actor loop and is joined, so the thread and socket are released deterministically.

## Risks / Trade-offs

- [One thread per client] → acceptable for a client; a connection pool or a fully async core
  can replace it later without changing the public async API.
- [A call hop adds latency] → negligible against network RTT; the API stays `async`.
