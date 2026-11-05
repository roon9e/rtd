# Design

## Context

The generated model exposes constructor ids and names (`types_map`) but no per-field descriptors.
Emitting them from `rtd-codegen` would add a very large generated table and a large diff. The
generated types already `derive(Debug)` with field names, which is exactly the shape `tdp` wants.

## Goals / Non-Goals

**Goals**

- A structural, field-named dump of any `Object`.
- A compact single-line form and a `name#id` label.

**Non-Goals**

- Decoding byte fields as base64 or formatting `date` fields as timestamps.

## Decisions

### D1. Use the generated `Debug` representation

`tdp::format` uses the alternate (`{:#?}`) `Debug` of the object, which prints the constructor and
its named fields recursively. This avoids a codegen change entirely and stays correct as the
schema evolves.

### D2. Keep a compact form and a label

`tdp::compact` uses the single-line `Debug`; `tdp::label` reuses `json::describe` for `name#id`.

## Risks / Trade-offs

- [Fidelity] `Debug` shows byte vectors as decimal lists, not base64, and dates as integers. This
  is documented as a non-goal; base64 can still be obtained from `json::format`/`encode_object`.
