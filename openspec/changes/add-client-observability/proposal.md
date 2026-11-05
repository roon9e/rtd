# Proposal

## Why

`td` ships `oteltg` (OpenTelemetry instrumentation) and a `clock/ntp` clock. `rtd-client` has an
abstract `Tracer` middleware but no OTel integration, and `rtd-util` has no NTP clock.

## What Changes

- Add an OpenTelemetry-backed `Tracer`/middleware behind an optional feature.
- Add an NTP-calibrated `Clock` behind an optional feature.

## Capabilities

### New Capabilities

- `client-observability`: OpenTelemetry instrumentation and an NTP clock.

## Impact

- **Crates**: `rtd-client`, `rtd-util`; optional deps (`opentelemetry`, an NTP crate).
- **Non-goals**: metrics beyond spans; a full OTel SDK setup.
