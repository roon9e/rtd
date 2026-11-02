# Proposal

## Why

File downloads/uploads dominate a Telegram edge's bandwidth, and routing them to the core wastes
a hop. gramsrv's edge serves `upload.getFile`/CDN directly from a file service
(`file_data` targets, `internal/mtprotoedge/filedata_layer_rpc.go`). `rtd-edge` forwards every
file call to the core.

## What Changes

- An edge-resident file-data path: intercept file methods (`upload.getFile`, `upload.getCdnFile`,
  hashes, `upload.saveFilePart`/`saveBigFilePart`) and proxy them to a file service over gRPC
  (`file_data` resolver/targets/token/timeout).
- Result re-wrap so the response is a normal MTProto result to the client.

## Capabilities

### New Capabilities

- `edge-file-fastpath`: edge-resident file-data serving.

## Impact

- **Crates**: `rtd-edge` (new `file` module, config), `rtd-edge-proto` (a file-service contract
  if gramsrv's differs).
- **Non-goals**: the file service itself (a separate binary).
