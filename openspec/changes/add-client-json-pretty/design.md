# Design

## Context

The generated model has `types_map()`; each object has a wire id and a name. `td`'s `tljson`
maps between JSON values and `tg.JSONValueClass` using the constructor name as `@type`. `tdp`
walks a type descriptor to print.

## Goals / Non-Goals

**Goals**

- Round-trip TL objects through JSON keyed by constructor name.
- A JSON `invoke` and a readable pretty-printer.

**Non-Goals**

- A full TDLib API emulation.

## Decisions

### D1. Name-keyed JSON with a registry

`tljson::encode` walks the value using the generated field names (snake_case) and writes
`@type: <name>`; `decode` looks up the name in `rtd_tmap`/`rtd_tg::types_map()` and decodes.
`InvokeJSON` decodes a method, invokes, and encodes the result or an error object.

### D2. `tdp` uses the generated type descriptors

`tdp::format` prints `Name { field: value }` by consulting per-type field metadata, avoiding
runtime reflection.

## Risks / Trade-offs

- [Field-name drift] → generate the name table from the same schema as the codecs (already the
  case) and add a round-trip test.
