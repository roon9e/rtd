# Proposal

## Why

Cross-DC delivery — routing a result/update to the DC the client is connected to, with replay and
acknowledgement — is a large part of gramsrv's edge (`rpc_result_egress.go`,
`delivery_executor.go`, `internal/egress`, `internal/edgecontrol/delivery_fabric.go`). `rtd-edge`
only returns results inline; it cannot deliver to another DC or replay after a drop.

## What Changes

- An egress delivery path: RPC-result egress over gRPC to an egress service
  (`egress.delivery` targets/token/timeout), with a delivery executor/codec.
- Replay and retention: restore in-flight deliveries after a reconnect, retain results until
  acknowledged, and bound retention.
- A bulk RPC scheduler and RPC flight tracking so many concurrent calls are delivered fairly.

## Capabilities

### New Capabilities

- `edge-delivery-fabric`: cross-DC result/update delivery with replay and retention.

## Impact

- **Crates**: `rtd-edge` (new `delivery` module), `rtd-edge-proto` (egress contract, or a new
  proto crate), building on `add-server-storage` for durable state.
- **Non-goals**: the egress and SFU services themselves.
