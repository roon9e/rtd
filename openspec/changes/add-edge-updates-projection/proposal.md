# Proposal

## Why

`rtd-edge` forwards methods but does not project or deliver updates (pts/qts/seq) to clients;
gramsrv's edge does (`internal/mtprotoedge/layer_updates.go`,
`channel_delivery_watermark.go`, `internal/egress/pts_projection_actor.go`). Without projection,
a reconnecting client cannot be caught up, and channel updates are not watermarked.

## What Changes

- A pts projection: accept update streams and deliver them to subscribers in order, tracking a
  per-client and per-channel watermark.
- A channel delivery watermark so reconnecting clients resume from their last confirmed position.
- Thin `layer_updates` handling so projected updates are encoded at the client's negotiated layer.

## Capabilities

### New Capabilities

- `edge-updates-projection`: ordered update projection with per-channel watermarks.

## Impact

- **Crates**: `rtd-edge` (or `rtd-server`), building on `add-edge-delivery-fabric`.
- **Non-goals**: the core's business rules for which updates to emit.
