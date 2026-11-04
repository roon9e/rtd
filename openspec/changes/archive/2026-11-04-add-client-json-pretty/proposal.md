# Proposal

## Why

Two developer-facing features are missing: MTProto JSON invoke (`InvokeJSON`/TDLib `@type`
JSON) and pretty-printing of TL objects (`tdp`/`tljson`). They make the client scriptable and
debuggable. `td` ships `telegram/tdlib_json.go`, `telegram/tljson`, `tdjson` and `tdp`.

## What Changes

- `tljson`/`tdjson`: encode/decode TL objects to/from JSON with an `@type` field.
- `InvokeJSON`: accept a JSON request in MTProto form, invoke it, return JSON.
- `tdp`: a reflection-free pretty-printer for TL objects (for logs/debug).

## Capabilities

### New Capabilities

- `client-json-pretty`: TL JSON coding, JSON invoke and pretty-printing.

## Impact

- **Crates**: `rtd-client` (or a small `rtd-json` module) plus a CLI path.
- **Non-goals**: the exact TDLib schema (only MTProto-form JSON).
