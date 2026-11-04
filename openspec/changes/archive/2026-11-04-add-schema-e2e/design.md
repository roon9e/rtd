# Design

## Context

`rtd-codegen` already parses a schema into a model and emits value/service/profile code. The
secret-chat and trace schemas are ordinary TL files; emitting them is a new output target, not a
parser change. `td` generates them as separate packages.

## Goals / Non-Goals

**Goals**

- Generated, committed Rust codecs for the encrypted and trace schemas.
- Provenance tracked like the canonical schema.

**Non-Goals**

- The secret-chat session state machine or MTProto routing for these types.

## Decisions

### D1. A separate generated crate per schema

`rtd-tg-e2e` and `rtd-tgtrace`, mirroring `td`'s package split, so they do not bloat the
canonical `rtd-tg`.

### D2. Reuse the existing emitter

The value-model emitter is reused with a different schema and crate name; only the module wiring
and `types_map` are new.

### D3. Budget check

Both crates are small relative to `rtd-tlprofile`; the generation-budget check applies to each.

## Risks / Trade-offs

- [Adding schema files needs provenance] → import them through the same manifest/policy path as
  the canonical schema.
