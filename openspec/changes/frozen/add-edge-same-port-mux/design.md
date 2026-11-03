# Design

## Context

Ingress currently inspects the first frame to classify the transport (reserved HTTP/TLS prefixes
are rejected unless WebSocket is enabled). gramsvr's `same_port_mux` peeks bytes, serves HTTP
separately, and hands the connection with its buffered prefix to the MTProto/WS path.

## Goals / Non-Goals

**Goals**

- One port for MTProto, WebSocket and HTTP.
- No byte loss on handoff.

**Non-Goals**

- TLS termination; server-info.

## Decisions

### D1. Peek-and-dispatch with a buffered connection

Read a small prefix; if it is an HTTP request line, serve HTTP (WebSocket upgrade to the WS
path); otherwise hand the prefix + socket to the MTProto path. Use a `PrefixedStream` so the
MTProto parser sees the peeking bytes.

### D2. Obfuscated2/FakeTLS win over HTTP when ambiguous

Because obfuscated2/FakeTLS bytes can look like arbitrary HTTP, only classify as HTTP when the
request line is valid after a bounded read; otherwise fall through to obfuscation detection
(extending the existing reserved-prefix logic).

### D3. HTTP routes are explicit

Only configured routes (health, metrics) are served; anything else is rejected without touching
the MTProto path.

## Risks / Trade-offs

- [Ambiguity between HTTP and obfuscated2] → bounded prefix read plus strict request-line
  validation; unresolved prefixes go to MTProto.
