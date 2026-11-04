# Proposal

## Why

`rtd-client` reads the socket only while a call awaits its reply: `Client::await_result` owns
the transport, so pushes and updates arriving between calls are never drained. `td` runs a
dedicated receive loop (`telegram/updates`, `mtproto` read loop) so the handler is genuinely
asynchronous; `rtd` cannot deliver unsolicited updates reliably until the transport is split.

## What Changes

- Split the client transport: add `ReadTransport`/`WriteTransport` (or `Transport::split`) so a
  reader thread can drain inbound frames while the caller writes.
- Implement the split for `Framed<TcpStream>` (via `try_clone`) and `DuplexStream` (its two
  channels already split).
- Put `ClientSession` behind a `Mutex` (or channel) so the reader and writer share sequence
  allocation safely.
- Have the reader emit `Result`/`Updates`/`Error` events to the caller through a channel or
  bounded queue, so `AsyncClient` updates are delivered without a pending call.

## Capabilities

### New Capabilities

- `client-background-receive`: a dedicated receive path that drains unsolicited server messages
  between calls.

## Impact

- **Crates**: `rtd-client` (`transport.rs`, `session.rs`, `client.rs`, `async_client.rs`).
- **Non-goals**: reconnection, keepalive and multi-salt (separate changes); the sync core
  simply gains an optional reader.
