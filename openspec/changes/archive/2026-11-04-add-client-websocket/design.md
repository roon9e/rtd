# Design

## Context

The server already speaks MTProto over WebSocket (`rtd-server::ingress`, `tokio-tungstenite`).
`td`'s client dials `wss://` and frames MTProto in binary WebSocket messages using the
intermediate codec. The client's `Transport` is byte-stream oriented, so a WS adapter that
implements `Read + Write` fits without changing the codec layer.

## Goals / Non-Goals

**Goals**

- `wss://`/`ws://` dialing that yields a normal framed client transport.
- Reuse the existing `Intermediate` codec and `Framed`.

**Non-Goals**

- WASM (no `std::net`); same-port mux; HTTP `http_wait`.

## Decisions

### D1. WS message framing over a byte-stream adapter

Binary WebSocket messages are concatenated into the stream; each `write` becomes one binary
message. A minimal `WsStream` implements `Read + Write` over `tokio-tungstenite` (spawned
runtime) or a blocking WS client, then `Framed` applies the intermediate codec.

### D2. Intermediate codec only

MTProto-over-WS always uses the intermediate codec (matching `td`); the transport is not
obfuscated.

## Risks / Trade-offs

- [Blocking the sync core on a tokio WS] → run the WS loop on the async façade and expose it to
  the sync core as a byte stream, or reuse the `add-client-background-receive` split.
