# Design

## Context

The generator emits codecs but not field descriptors. `tdp` needs `(name, kind)` per field to
render values without runtime reflection.

## Goals / Non-Goals

**Goals**

- A stable, generated field-metadata table keyed by wire id.
- A printer that renders `Name { field: value, ... }`.

**Non-Goals**

- Round-tripping through the metadata.

## Decisions

### D1. Field metadata as a generated static

Emit `fn fields(id) -> Option<&'static [Field]>` with `Field { name, kind }`, where `kind`
distinguishes scalars, strings, bytes, vectors, flags and nested objects.

### D2. Printer is tolerant

Unknown kinds render as `<id>`; recursion is depth-bounded.

## Risks / Trade-offs

- [Generated size] The table adds code; keep names as `&'static str` and avoid per-field allocs.
