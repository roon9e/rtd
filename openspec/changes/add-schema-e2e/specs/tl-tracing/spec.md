# Spec Delta

## Purpose

Defines a generated tracing value model.

## ADDED Requirements

### Requirement: Tracing schema is generated

The generator SHALL import `_schema/trace.tl` and emit committed Rust codecs for its types.

#### Scenario: A trace object round-trips
- **WHEN** a trace object is encoded and decoded
- **THEN** the decoded value equals the original
