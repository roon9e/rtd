# Design

## Context

A client is connected to one edge; a result produced elsewhere must reach it. gramsrv models this
as a delivery fabric: the edge receives an RPC result/update, enqueues it for the owning
connection's DC, and retries until the client acknowledges. `internal/egress` handles the
projection and wake loop; `edgecontrol` tracks active auth keys and delivery.

## Goals / Non-Goals

**Goals**

- Deliver results/updates that did not originate on the client's connection.
- Survive a reconnect: replay unacknowledged deliveries.
- Bound concurrency and retention.

**Non-Goals**

- The egress service implementation; SFU.

## Decisions

### D1. A `Delivery` abstraction over gRPC

`Delivery::enqueue(target, envelope)` and `Delivery::drain()`; a `GrpcDelivery` dials the
configured egress targets with a bearer token. This mirrors the core-exec adapter.

### D2. At-least-once with ack evidence

Deliveries carry a sequence/id; the edge retains them until the client's `msgs_ack` (client-ack
evidence) confirms receipt, then prunes. Retention is bounded by count and age (durable in
`add-server-storage`).

### D3. Fair scheduling with rpc-flight tracking

A bulk scheduler and rpc-flight map pick the next delivery per connection, preventing one busy
connection from starving others.

## Risks / Trade-offs

- [Duplicate delivery] → clients ignore duplicates by message id; keep at-least-once and idempotent
  projection, as the multi-DC spec requires.
