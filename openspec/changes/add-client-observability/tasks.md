# Tasks

## 1. OpenTelemetry

- [ ] 1.1 Add an OTel-backed tracer implementing the existing `Tracer` trait
- [ ] 1.2 Gate it behind a non-default feature

## 2. NTP clock

- [ ] 2.1 Add an NTP-calibrated `Clock` behind a non-default feature

## 3. Verification

- [ ] 3.1 Verify the tracer records a span per call
- [ ] 3.2 Verify the NTP clock offsets the system time by the measured delta
