# Design

## Context

The actor and router have natural metric points; `rtd-edge` already runs tokio and can host a
small HTTP endpoint. gramsrv uses an internal metrics registry and a debug addr.

## Goals / Non-Goals

**Goals**

- Low-overhead counters/histograms with a stable text exposition.
- Identity visible in logs and the core-exec boundary.

**Non-Goals**

- Long-term storage or alerting.

## Decisions

### D1. A small registry, not a heavy SDK

`Metrics` is a registry of atomic counters and simple histograms; exposition is Prometheus text
format over a plain HTTP handler (`hyper`/`axum` or a hand-written minimal server), mirroring
gramsrv's approach and avoiding a large dependency.

### D2. Identity is generated once and logged

`InstanceId` (random or from config) plus build info (git SHA, version) is logged at startup and
sent as core-exec metadata where useful.

### D3. Tracing hook integrates with client middleware

Expose a `Tracer` trait used by both the server dispatch and the client middleware no-op, so one
implementation can be plugged into both.

## Risks / Trade-offs

- [Label cardinality] → restrict labels to method class and outcome, never peer ids.
