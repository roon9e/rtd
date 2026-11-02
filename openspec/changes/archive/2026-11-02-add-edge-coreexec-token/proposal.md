# Proposal

## Why

`rtd-edge` is the client-facing MTProto edge that fronts a core over
`telesrv.coreexec.v1.CoreExecService`. A real gramsrv v2 core authenticates the edge with a
shared bearer token (`TELESRV_CORE_EXEC_TOKEN`) sent as gRPC metadata
(`authorization: Bearer <token>`); the pinned proto has no token field, so the token only
travels in metadata. `rtd-edge` currently sends no metadata, so a token-protected core rejects
every call with `Unauthenticated`. This is the main blocker to running `rtd-edge` as a drop-in
replacement for a gramsrv v2 edge.

## What Changes

- An optional core-exec bearer token in [`EdgeConfig`] (`--core-token` /
  `RTD_EDGE_CORE_TOKEN`), mapped from gramsrv's `TELESRV_CORE_EXEC_TOKEN`.
- [`GrpcCore`] attaches `authorization: Bearer <token>` to every unary call when a token is
  configured, so a real core accepts the edge.
- A `Dockerfile` for `rtd-edge`, so the edge can be built as a container image and swapped into
  a gramsrv v2 compose stack.

## Capabilities

### New Capabilities

- `edge-service`: the edge's core-exec credential and its container packaging.

## Impact

- **Crates**: `rtd-edge` only; no protocol or contract change (the metadata header is invisible
  to the protobuf schema).
- **Non-goals**: edge-side identity server-info, file/egress fastpaths, redis, and multi-DC.
- **Compatibility**: unchanged against an untokened core (no metadata is sent).
