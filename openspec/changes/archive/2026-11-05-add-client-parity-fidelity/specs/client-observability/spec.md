# Spec Delta

## Purpose

Extends observability with OpenTelemetry metrics.

## ADDED Requirements

### Requirement: OpenTelemetry metrics

The client SHALL provide an optional tracer that counts invocations per method and outcome as an
OpenTelemetry counter.

#### Scenario: Call is counted
- **WHEN** a call is invoked with the OTel metrics recorder installed
- **THEN** the invocation counter is incremented with the method and outcome attributes
