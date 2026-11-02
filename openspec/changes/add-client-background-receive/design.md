# Design

## Context

Today `Client<T: Transport>` owns one `Transport`; `call`/`await_result` alternate frames on
it. Updates that arrive outside a call remain buffered in the socket until the next call reads
them. `td` instead runs a reader goroutine and routes inbound messages (RPC results, updates,
service messages) to consumers while writes happen independently.

## Goals / Non-Goals

**Goals**

- A receive path independent of any in-flight call.
- Deterministic tests over `DuplexStream` with no socket.

**Non-Goals**

- Reconnect/backoff, ping scheduling, multi-salt refresh; these build on this change.

## Decisions

### D1. Split by capability, not by duplicating state

Introduce `ReadTransport::recv_frame` and `WriteTransport::send_frame`, with a blanket
`Transport::split`. `Framed<TcpStream>` splits via `TcpStream::try_clone` (two independent
handles); `DuplexStream` hands each end its existing channel. This avoids a second connection
and keeps framing intact.

### D2. Session behind a lock

Message-id/seq allocation and inbound-ack bookkeeping move behind a `Mutex<ClientSession>` (or
a small actor channel). The hot path stays lock-free per direction; the lock is held only for
sequence allocation and decode.

### D3. Events, not callbacks inside the reader

The reader thread sends `ClientEvent::{Result, Update, Error}` over a channel. The sync core
exposes `recv_event(timeout)`; `AsyncClient` bridges it to `tokio::sync::mpsc`, replacing the
current per-call read ownership.

## Risks / Trade-offs

- [Two TcpStream handles can diverge on close] → treat either side's error as terminal and
  signal `Closed` once.
- [Reader must not spin] → block on `recv_frame`; no polling loop.
