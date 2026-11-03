# Proposal

## Why

`rtd-edge --websocket` accepts any WebSocket origin. gramsrv's edge configures
`websocket.allowed_origins`, and a browser deployment needs origin checking. Without it, a
malicious page could attempt to open MTProto WebSocket connections.

## What Changes

- A WebSocket origin allowlist (`--websocket-origin`, repeatable) enforced during the upgrade.
- Reject an upgrade whose `Origin` is not allowed when the allowlist is non-empty.
- Keep a permissive default only when no allowlist is configured (matching current behavior).

## Capabilities

### New Capabilities

- `edge-websocket-origins`: WebSocket origin allowlisting.

## Impact

- **Crates**: `rtd-server` (`ingress.rs`), `rtd-edge` (config).
- **Non-goals**: same-port mux (separate change).
