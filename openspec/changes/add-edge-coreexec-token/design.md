# Design

## Context

gramsrv v2's `internal/coreexec/grpc.go` sends `authorization: Bearer <token>` via a unary
client interceptor and rejects a mismatched or missing bearer on the server with
`codes.Unauthenticated`. The token is configuration, not schema, so `rtd-edge-proto` (a pin of
the proto) is unchanged. `rtd-edge`'s [`GrpcCore`] performs unary calls with
`tonic::client::Grpc`, which exposes per-call metadata.

## Goals / Non-Goals

**Goals**

- Send the configured bearer token as gRPC metadata on every `CoreExecService` call.
- Package the edge as a container so a gramsrv v2 compose stack can serve with it.

**Non-Goals**

- TLS/mTLS core transport, request-id metadata, and the edge-side capabilities gramsrv's Go
  edge has beyond MTProto admission and dispatch.

## Decisions

### D1. The token is optional configuration, sent only when set

`EdgeConfig` gains `core_token: Option<String>` from `--core-token`/`RTD_EDGE_CORE_TOKEN`.
`GrpcCore` stores the token and, when it is `Some`, inserts
`authorization: Bearer <token>` into each request's metadata. When it is `None` the edge sends
no metadata, so it still works against an untokened core (and the existing tests).

### D2. Metadata is attached per request, not by a tonic interceptor

`GrpcCore::unary` already builds each `tonic::Request`, so the token is inserted there
(`request.metadata_mut().insert(...)`) rather than in a client interceptor. This keeps the
adapter self-contained and testable without a live channel.

### D3. A container that runs the static binary

The `Dockerfile` builds `rtd-edge` in a Rust builder stage and copies the binary into a slim
runtime that runs non-root, matching the permissions model of the gramsrv compose stack. The
RSA key and the core endpoint/token are supplied at run time.
