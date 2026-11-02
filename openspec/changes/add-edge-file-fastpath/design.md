# Design

## Context

gramsrv's `edge.yaml` declares `file_data` (resolver, targets, token, timeout). The edge detects
file methods after admission and calls the file service, then wraps the result, mirroring how the
core is called. Layer handling for file methods is done at the edge
(`filedata_layer_rpc.go`).

## Goals / Non-Goals

**Goals**

- File methods bypass the core, cutting latency and core load.
- The token and transport mirror the core-exec path.

**Non-Goals**

- Any change to the file storage service.

## Decisions

### D1. A `FileService` trait parallel to `Core`

`FileService::get_file(...) -> Result<Bytes>` over gRPC; a `GrpcFileService` dials the configured
targets with the bearer token, reusing the core-exec metadata machinery.

### D2. Route by method id after admission

The router classifies file methods and dispatches them to `FileService`, wrapping the result in
`rpc_result` exactly as the core path does.

### D3. Layer-aware, like core-exec

File responses are encoded at the client's negotiated layer using `rtd-tlprofile`, consistent
with `add-edge-*` layer handling.

## Risks / Trade-offs

- [Partial-file edge cases] → fall back to the core when the file service errors, so correctness
  is not sacrificed for speed.
