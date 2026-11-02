# Proposal

## Why

`rtd-edge` is configured with `--flag`/`RTD_EDGE_*`, while the gramsrv v2 stack uses `edge.yaml`
plus `TELESRV_*` environment variables and runs several services (core, egress, sfu, file, redis).
A drop-in needs in-repo config parity and a compose overlay so it can be swapped in without
external glue.

## What Changes

- A YAML config loader for `edge.yaml` with `TELESRV_*` interpolation, mapped onto `EdgeConfig`.
- In-repo compose overlay(s) and example configs so the edge runs in the gramsrv v2 stack.
- Redis wiring for the coordination the edge needs.

## Capabilities

### New Capabilities

- `edge-config-parity`: gramsrv-compatible edge configuration and compose overlay.

## Impact

- **Files**: `rtd-edge` config loader, `deploy/docker/**`, `configs/examples/**`.
- **Non-goals**: publishing the image (already covered by the publish workflow).
