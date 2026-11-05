# client-observability Specification

## Purpose
Defines optional OpenTelemetry instrumentation and an NTP-calibrated clock.

## Requirements

### Requirement: OpenTelemetry tracing

The client SHALL provide an optional tracer that records one span per invoked call, carrying the
method name and outcome.

#### Scenario: Call is traced
- **WHEN** a call is invoked with the OTel tracer installed
- **THEN** a span is recorded for it

### Requirement: NTP clock

The runtime SHALL provide an optional clock calibrated against an NTP server.

#### Scenario: Offset is applied
- **WHEN** the NTP clock has measured a server offset
- **THEN** the clock's now returns the system time adjusted by that offset

### Requirement: OpenTelemetry metrics

The client SHALL provide an optional tracer that counts invocations per method and outcome as an
OpenTelemetry counter.

#### Scenario: Call is counted
- **WHEN** a call is invoked with the OTel metrics recorder installed
- **THEN** the invocation counter is incremented with the method and outcome attributes
