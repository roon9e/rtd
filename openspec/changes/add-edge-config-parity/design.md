# Design

## Context

gramsrv's `edge.yaml` has `instance`, `debug`, `public`, `redis`, `edge` (listen, rsa_key,
websocket, mtproto, location, caches), `identity`, `core_exec`, `file_data`, `egress`. The rtd
edge already supports `--core`/`--core-token`; the rest is config mapping.

## Goals / Non-Goals

**Goals**

- Load the same `edge.yaml` shape and `TELESRV_*` env into `EdgeConfig`.
- Run in the compose stack via an in-repo overlay.

**Non-goals**: re-implementing gramsrv business logic.

## Decisions

### D1. YAML + env expansion onto `EdgeConfig`

A small YAML loader expands `${TELESRV_*}` and maps each section to the corresponding
`EdgeConfig` field, reusing the existing flags as the low-level knobs.

### D2. In-repo overlay

`deploy/docker/` gains a compose file that builds `rtd-edge` from the Dockerfile and points it at
the stack's core/file/egress, plus `configs/examples/edge.local.yaml` mirroring gramsrv.

### D3. Feature-gated imports

The loader and compose are tooling, not runtime behavior; they must not weaken the sans-IO
boundary or add `unsafe`.

## Risks / Trade-offs

- [Config drift with gramsrv] → pin the source ref in comments and validate the loaded config.
