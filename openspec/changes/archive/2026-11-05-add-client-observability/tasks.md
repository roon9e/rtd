# Tasks

## 1. OpenTelemetry

- [x] 1.1 Add an OTel-backed tracer implementing the existing `Tracer` trait (`OtelTracer`)
- [x] 1.2 Gate it behind a non-default `otel` feature

## 2. NTP clock

- [x] 2.1 Add an NTP-calibrated `Clock` behind a non-default `ntp` feature (`rtd-util::ntp`)

## 3. Verification

- [x] 3.1 Verify the OTel tracer records a span per call (no-op global provider)
- [x] 3.2 Verify the offset math against synthetic SNTP replies
