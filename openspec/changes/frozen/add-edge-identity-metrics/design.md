# Design

## Context

`add-server-observability` defines a registry and dispatch metrics. The edge needs identity and
runtime-level metrics to be operable, and gramsrv's edge has both plus a debug addr.

## Goals / Non-Goals

**Goals**

- Unique, stable-per-process identity and build info.
- Runtime counters usable without a profiling session.

**Non-Goals**

- Distributed tracing backend.

## Decisions

### D1. Identity from config or random

`instance.id` comes from config/env when set, else a random id generated at startup and logged.
Build info is compiled in (`option_env!` for the SHA/version).

### D2. Identity is sent on boundary calls

The core-exec and egress clients attach the instance id as gRPC metadata, so the core can
attribute calls (gramsrv does the same).

### D3. Runtime metrics on the existing endpoint

Connection/session/in-flight/byte counters are registered in the shared registry and exposed on
the observability endpoint from `add-server-observability`; the debug addr optionally enables a
profiler.

## Risks / Trade-offs

- [Identity churn on restart] → a configured id is stable; a random one is logged so reconnects
  are attributable.
