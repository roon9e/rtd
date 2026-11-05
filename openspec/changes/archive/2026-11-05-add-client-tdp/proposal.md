# Proposal

## Why

`td`'s `tdp` renders a TL value as a readable, field-by-field dump. `rtd-client::json::format`
only prints `name#id` plus an opaque base64 body, so debugging a response means decoding it
elsewhere.

## What Changes

- Add a `tdp` module that pretty-prints any [`rtd_tg::Object`] structurally, using the generated
  `Debug` representation (constructor name and named fields).
- Keep the compact `name#id` label available as a fallback.

## Capabilities

### New Capabilities

- `client-tdp`: structural TL pretty-printing.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: a reflection API, byte-field base64 rendering, and date formatting (the generated
  model exposes neither field metadata nor typed dates).
