# Proposal

## Why

`td`'s `tdp` renders any TL value as a readable, field-by-field dump (with `[]byte` as base64 and
dates formatted). `rtd-client::json::format` only prints `name#id` plus an opaque body, because
the generated model carries no per-field descriptors.

## What Changes

- Emit per-type field metadata from `rtd-codegen` (name + kind per field).
- Add a structural pretty-printer that walks that metadata.

## Capabilities

### New Capabilities

- `client-tdp`: structural TL pretty-printing.

## Impact

- **Crates**: `rtd-codegen`, `rtd-tg`, `rtd-client`.
- **Non-goals**: a full reflection API; only what the printer needs.
