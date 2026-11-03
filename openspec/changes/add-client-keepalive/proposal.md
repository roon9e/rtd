# Proposal

## Why

`rtd-client` never pings and never reconnects: a stalled connection hangs forever and an idle
connection is dropped by NATs. `td` sends `ping`/`ping_delay_disconnect` on a timer and
reconnects with backoff, retrying requests not processed by the server
(`mtproto/ping.go`, `telegram/ping.go`, `telegram/connect.go`).

## What Changes

- A keepalive loop sending `ping` (`ping_delay_disconnect` while connected) at an interval and
  tracking `pong` latency.
- Reconnect with exponential backoff on error/idle, reusing the persisted auth key.
- Retry of requests that were sent but not acknowledged, or not yet sent, after reconnect.
- Integration with the background reader and the multi-salt store.

## Capabilities

### New Capabilities

- `client-keepalive`: periodic pings, disconnect detection and reconnection with retry.

## Impact

- **Crates**: `rtd-client` (`client.rs`, `async_client.rs`), depends on `add-client-background-receive`
  and `add-client-multi-salt`.
- **Non-goals**: DC migration on reconnect and pooling (separate changes).
