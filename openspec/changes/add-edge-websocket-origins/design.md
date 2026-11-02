# Design

## Context

The edge's WebSocket upgrade is handled by `tokio-tungstenite` inside `rtd-server::ingress`. The
HTTP `Origin` header is available at upgrade time. gramsrv lists allowed origins in config.

## Goals / Non-Goals

**Goals**

- Enforce an origin allowlist without breaking non-browser clients (which send no `Origin`).

**Non-Goals**

- CORS for HTTP routes.

## Decisions

### D1. Allowlist checked at upgrade

`IngressConfig.websocket_origins: Vec<String>`; if non-empty, an upgrade must carry an allowed
`Origin`. A missing `Origin` (non-browser client) is allowed only when the list is empty.

### D2. Exact-origin match

Match the full origin (`scheme://host[:port]`) exactly, case-insensitively, to avoid
substring bypasses.

## Risks / Trade-offs

- [Breaking legitimate embeds] → default empty list preserves the current permissive behavior;
  operators opt in.
