# Proposal

## Why

`rtd-server` accepts WebSocket ingress, but `rtd-client` has no WebSocket transport, so a client
can only use TCP. `td` ships a WebSocket transport (`transport/websocket.go`), a `dcs.Websocket`
resolver and `wsutil`, and its client works from WASM.

## What Changes

- Add a client-side WebSocket transport (MTProto over WS, intermediate codec), reusing the
  existing `Transport`/`Framed` abstractions by presenting the socket as a byte stream.
- Provide a `Dialer` for `ws://`/`wss://` endpoints and wire it into the connect path.
- Reject non-binary frames and bound the handshake.

## Capabilities

### New Capabilities

- `client-websocket`: MTProto-over-WebSocket transport for the client.

## Impact

- **Crates**: `rtd-client` (`transport.rs`), optionally a small `wsutil`-equivalent module.
- **Non-goals**: same-port multiplexing (an edge change) and WASM support.
