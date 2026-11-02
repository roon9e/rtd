# Proposal

## Why

`rtd` has no metrics, tracing or instance identity, so an operator cannot see throughput,
handshake failures, queue depth or which edge instance served a request. gramsrv exposes
`observability/metrics`, a debug endpoint and `instance.id`; `td` ships `oteltg`.

## What Changes

- A metrics registry and counters/histograms (connections, handshakes, admissions, dispatches,
  errors, latency) exposed over an HTTP endpoint.
- Instance identity (unique id/build info) reported in logs and headers.
- A tracing hook compatible with the client middleware hook point.
- A configurable debug/observability listen address.

## Capabilities

### New Capabilities

- `server-observability`: metrics, tracing hooks and instance identity.

## Impact

- **Crates**: `rtd-server` (metrics facade), `rtd-edge` (HTTP endpoint, config, build info).
- **Non-goals**: a hosted dashboard.
