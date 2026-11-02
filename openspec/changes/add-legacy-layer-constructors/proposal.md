# Proposal

## Why

`complete-multi-layer` makes every route whose definition exists in both canonical 232 and the
profile executable. A `ProfileOnly` definition exists **only** in an older layer
(`keyboardButtonRequestPhone`, the older `replyInlineMarkup` row buttons, …): canonical 232
has no value for it, so it has no canonical slot to convert from or to. Those routes
currently fail closed, and the one present-in-both hook that transitively depends on them
(`layerAdaptLegacyInlineKeyboardRows`) is deferred here (design D37 / D41 of
`complete-multi-layer`).

## What Changes

- **Per-profile legacy codecs**: the generator emits a field codec for every `ProfileOnly`
  constructor, so a value that exists only in an older layer can be decoded into the profile
  model and re-encoded, with no canonical counterpart.
- **The 17 `old-only` adapter hooks** are implemented and emitted (`rtd-layerhooks`), and the
  deferred `layerAdaptLegacyInlineKeyboardRows` row adapter is implemented over the legacy
  button constructors.
- **Fail-closed clause removed**: a `ProfileOnly` route is served instead of refused, so the
  `multi-layer-tl` "profile-only fails closed" clause is retired (MODIFIED requirement once
  `complete-multi-layer` is archived).
- **Profile model for legacy types**: the generated sidecar carries the legacy constructor
  shapes it needs; the canonical model is unchanged (canonical layer stays 232).

## Capabilities

### New Capabilities

- `legacy-layer-constructors`: per-profile codecs and conversion hooks for constructors that
  exist only in an older layer, so a `ProfileOnly` route is served rather than refused.

### Modified Capabilities

- `multi-layer-tl`: `ProfileOnly` routes are served, not refused (removes the fail-closed
  clause added by `complete-multi-layer`).
- `layer-conversion-policy`: the `old-only` hooks and the row adapter execute.

## Impact

- **Crates**: `rtd-codegen` gains per-profile legacy constructor emission; `rtd-layerhooks`
  gains the `old-only` hooks and the row adapter; `rtd-tlprofile` grows.
- **Schema/config**: canonical layer stays 232; provenance unchanged.
- **Compatibility**: older-layer traffic that was refused becomes served; canonical wire
  behavior is unchanged.
