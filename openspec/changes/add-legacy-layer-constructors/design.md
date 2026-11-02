# Design

## Context

`complete-multi-layer` implements the present-in-both machinery: exact-profile admission,
route classification, the shared hook-signature registry and its coverage gate, the
mechanical conversions, and hook call-site emission for the conversion, requirement,
supply, capture and project shapes. It treats a `ProfileOnly` definition as unavailable
(fail closed) and defers this change (its D37/D41).

A `ProfileOnly` constructor has no canonical definition, so the canonical value model has no
type for it and there is no "convert to canonical" step. It must instead be decoded and
encoded directly against the profile's own field layout, and the hooks that adapt it are the
`old-only` ones (e.g. `layerAdaptLegacyKeyboardButtonRequestPhone`).

## Goals / Non-Goals

**Goals**

- Serve `ProfileOnly` routes on the enabled layers: decode into the profile model and
  re-encode, with the reviewed `old-only` conversions.
- Keep canonical 232 and its wire behavior unchanged.

**Non-Goals**

- No multi-DC; no client library; no canonical-layer change.

## Decisions

### D1. Per-profile legacy constructors are generated, not hand-written

The generator already computes `ProfileOnly` families and their field layouts. Emit a codec
for each such constructor in the older layer's own model (a per-profile legacy value type),
so decoding and encoding a `ProfileOnly` route is generated field logic like the present-in-
both routes. Hand-writing 73 policy entries' worth of codecs is rejected as unauditable.

### D2. The `old-only` hooks are field-level, in `rtd-layerhooks`

The 17 `old-only` hooks convert a legacy constructor to and from the canonical class it
replaced (`keyboardButtonRequestPhone` <-> `keyboardButton` + `buttonTypeRequestPhone`), with
the same fail-closed discipline as the present-in-both hooks. The registry marks them
`Implemented` and the generation gate enforces coverage exactly as it does today.

### D3. The row adapter composes the button hooks

`layerAdaptLegacyInlineKeyboardRows` converts `Vector<Row>` element by element, using the
per-button `old-only` hooks; a button that cannot be represented fails closed. This is the
only present-in-both hook deferred from `complete-multi-layer`.

### D4. Removing the fail-closed clause

`multi-layer-tl`'s "profile-only fails closed" clause is retired with a MODIFIED requirement
once `complete-multi-layer` is archived; until then this change's spec delta ADDs the
`legacy-layer-constructors` capability only.

### D5. The rows codec is generator-emitted field logic over field-value hooks

The `rows` adapter is option B: the generator reads the older layer's `KeyboardButtonRow` and
`KeyboardButton` layouts from the schema, emits the field reads/writes, and calls the
registered `old-only` field-value hooks (each takes/returns the legacy field values in the
schema's declaration order, matching `td`). The button hooks stay pure value transforms and do
not depend on `rtd-bin`; the wire layout has a single source of truth (the `.tl`). Because
`layerAdaptLegacyInlineKeyboardRows` is emitted as a nested codec rather than a hand-written
function, its registry status is `Generated`: generation may reference it, but
`rtd-layerhooks` does not implement it. Each `old-only` hook's schema field order is pinned by
a generation-time test so a layer update cannot silently misalign it.

## Risks / Trade-offs

- [Legacy codecs inflate the sidecar] → Generate only the constructors a `ProfileOnly` route
  needs and deduplicate; record any budget raise.
- [A legacy hook diverges from `td`] → The policy key/digest gate admits only reviewed
  differences; port each hook from `td/tlprofile/adapters.go` and pin it with a test.
- [Removing fail-closed weakens safety] → Fail-closed is preserved for genuinely
  unrepresentable values; only reviewed `ProfileOnly` routes become served.

## Open Questions

- Whether to split the row adapter from the legacy constructors if the surface proves large.
