# Design

## Context

gramsrv's egress projects updates to edges; the edge delivers them and tracks a per-channel
watermark so a client that reconnects gets everything after its last confirmed position. Updates
must be encoded at the client's layer.

## Goals / Non-Goals

**Goals**

- In-order, at-least-once update delivery per client.
- Correct resume after reconnect.

**Non-Goals**

- Deciding update content (the core does).

## Decisions

### D1. Watermarks per client and per channel

A watermark records the highest delivered/acknowledged pts (and channel pts). On reconnect, the
edge resumes from the watermark, requesting a `getDifference` gap-fill from the core if needed.

### D2. Layer encoding at delivery time

Updates are encoded with `rtd-tlprofile` for the session's negotiated layer, so a 229 client and
a 232 client both decode correctly.

### D3. Reuse the delivery fabric

Projection reuses `add-edge-delivery-fabric`'s retention/replay rather than a second queue.

## Risks / Trade-offs

- [Ordering across channels] → per-channel watermarks plus the global seq keep ordering within a
  channel while permitting parallel channels.
