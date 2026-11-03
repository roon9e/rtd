# Proposal

## Why

`rtd-edge` detects the ingress transport (plain / obfuscated2 / FakeTLS / WebSocket) but does not
multiplex HTTP on the same port, so a browser client, a health probe or an HTTPS-style client
cannot share the MTProto port. gramsrv v2's edge runs a same-port mux
(`internal/mtprotoedge/same_port_mux.go`) that distinguishes MTProto, WebSocket and HTTP.

## What Changes

- A same-port multiplexer that peeks the first bytes, routes WebSocket upgrades to the WS path,
  plain HTTP to the HTTP handler (health/metrics), and everything else to the MTProto path.
- Transport ownership so the chosen path reads the buffered prefix without loss.
- Reserved-prefix handling so obfuscated2/FakeTLS connections are not misclassified as HTTP.

## Capabilities

### New Capabilities

- `edge-same-port-mux`: one listening port shared by MTProto, WebSocket and HTTP.

## Impact

- **Crates**: `rtd-server` (`ingress.rs`) and `rtd-edge`.
- **Non-goals**: the `/owpengram/server-info` descriptor (skipped).
