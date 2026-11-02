# Spec Delta

## Purpose

Defines gramsrv-compatible edge configuration and a compose overlay.

## ADDED Requirements

### Requirement: Compatible configuration

The edge SHALL load a gramsrv-shaped YAML configuration with `TELESRV_*` expansion and map it onto
its runtime configuration, with flags as overrides.

#### Scenario: Example config loads
- **WHEN** the example `edge.yaml` is loaded
- **THEN** every section maps to the corresponding edge option

### Requirement: Compose overlay

The repository SHALL provide a compose overlay that runs the edge in the gramsrv v2 stack.

#### Scenario: Edge runs in the stack
- **WHEN** the overlay is brought up with the stack
- **THEN** the edge starts, dials the core, and completes `help.getConfig`
