# Proposal

## Why

gramsrv's edge exposes an instance identity (`instance.id`), build info and runtime metrics
(`internal/mtprotoedge/runtime_metrics.go`, `metrics.go`) and a debug address. `rtd-edge` logs
almost nothing and has no identity, so an operator cannot attribute traffic or correlate logs.
`add-server-observability` adds the registry and dispatch instrumentation; this change makes the
edge binary expose identity and runtime metrics.

## What Changes

- An instance identity (config or random) and build info (`git` SHA, version) in a
  `--version`/startup banner and a `/buildinfo` route.
- Runtime metrics: connections, sessions, in-flight RPCs, outbound bytes, and a debug addr for
  profiling.
- Correlate core-exec/egress calls with the instance id.

## Capabilities

### New Capabilities

- `edge-identity-metrics`: edge instance identity, build info and runtime metrics.

## Impact

- **Crates**: `rtd-edge` (`config.rs`, `service.rs`, `main.rs`); depends on
  `add-server-observability` for the registry.
- **Non-goals**: a hosted dashboard.
