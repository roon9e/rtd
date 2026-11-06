# Proposal

## Why

Telegram Desktop's stable branch landed **TL layer 233** (`ready@a6df181`), and the `td`
reference has already moved its canonical layer to 233. `rtd` must track it: the canonical layer
gates admission, per-profile encoding and the client's `invokeWithLayer`.

## What Changes

- Import `layer-233.tl` (locked commit/blob/SHA-256) and move `canonical_layer` to 233 in
  `schema/layers/manifest.json` and `schema/rtd.json`; enable 225–233.
- Adopt `td`'s audited `policy.json` (305 entries; 56 new obligations, all `drop`/`reject`, no
  hooks).
- Regenerate `rtd-tg` and `rtd-tlprofile`; raise the generated-source line budget to fit the
  ninth profile.

## Capabilities

### Modified Capabilities

- `multi-layer-tl`: canonical layer becomes 233.

## Impact

- **Crates**: `rtd-codegen`, `rtd-tg`, `rtd-tlprofile` (generated), `rtd-client` (fixtures).
- **Non-goals**: the frozen edge keeps its configured layer (229/232); the MTProto service schema
  is unchanged in 233.
