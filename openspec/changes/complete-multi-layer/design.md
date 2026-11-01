# Design

## Context

See `proposal.md` — Why. What shapes the approach:

- `schema/rtd.json` enables only `[232]`; `schema/layers/manifest.json` already lists layers
  225–232 with tdesktop provenance, and `policy.json` holds 249 reviewed entries (34
  distinct adapter hooks; actions `adapter` 121, `reject-if-present` 60, `reject` 35,
  `drop` 21, `project` 7, `default` 5).
- `rtd-codegen` already computes everything needed: `obligations` produces a
  `ConversionPlan` (per-definition conversions, field ordinals, obligations with their
  reviewed resolution), `routes` classifies the full `(wire × profile)` product into the six
  modes and builds deduplicated body/preflight/result `PlanSet`s. But
  `emit_profile::emit` takes `_plan`/`_plans` unused, so nothing is emitted for a
  non-direct route.
- The generated `rtd-tlprofile` runtime executes `Direct` only: `admit_impl` returns
  `UnsupportedRoute`, `encode_route` returns `RouteNotEmitted`, and `decode_object`
  rejects any non-direct route. `FrozenObject::encode` routes through `encode_route`.
- The profile sidecar is currently ~0.79 MB / ~22k lines; a declared size/line budget is
  enforced.
- Frozen decisions in play: multi-layer semantics (exact-profile admission, fail-closed,
  no runtime schema, no canonical transcode bridge), generated code committed and
  never hand-edited, deterministic/atomic output, and the reviewed-policy gate.

## Goals / Non-Goals

**Goals**

- Layers 225–232 admitted; `Present` non-direct routes (retag/rewrite/policy) executed per
  the reviewed conversion; fail-closed for unavailable routes and, for now, profile-only
  routes.
- Generated code stays never-hand-edited; conversion logic that cannot be generated lives
  in a hand-written crate the generated code calls, driven by a shared hook-signature
  registry.
- Deterministic, atomic, budgeted generation over the full enabled set.

**Non-Goals**

- No canonical-layer change (232 stays canonical) and no new schema import.
- No runtime schema interpretation, no byte↔byte canonical transcoder, no canonical
  fallback.
- `ProfileOnly` constructors (defined only in an older layer) are deferred to the
  follow-up change `add-legacy-layer-constructors`; this change treats them as unavailable.
- Not multi-DC, not a client library.

## Decisions

### D29. Conversion hooks live in a new hand-written crate, `rtd-layerhooks`

The 34 adapter hooks are hand-written Rust (there is no source language to generate them
from). The generated `rtd-tlprofile` must not be hand-edited, so the hooks live in a new
crate, `rtd-layerhooks`, that depends only on `rtd-tg`/`rtd-mt`; the generated sidecar
depends on it and calls it for policy routes. The hook interface takes the canonical value
plus the obligation context and returns a converted value or a fail-closed error.

Alternatives: a hand-written module file inside `rtd-tlprofile` beside the generated
`lib.rs` (rejected — it makes a "generated, never hand-edited" crate partly hand-written and
complicates the freshness check); generating hooks as data (rejected — the conversions are
real logic, not tables).

### D30. Emission strategy: bodies as deduplicated plans, dispatched by `(profile, wire_id)`

`emit_profile` stops ignoring the plan set. For each non-direct route it emits a body:
retag and rewrite inline from the conversion plan; profile-only encodes the value as-is;
policy calls the hook registry by obligation key; unavailable emits nothing (its absence is
the action). Bodies are deduplicated by the plan digest already computed, so one shared
body serves many routes, and the runtime dispatches through the existing route table
extended with a body-plan id. Admission, `encode_route` and `decode_object` consult the
body instead of rejecting.

Alternatives: a per-route function per `(profile, wire_id)` (rejected — explodes the
sidecar and loses dedup); interpreting the conversion plan at runtime (rejected — the plan
is build-time data; runtime interpretation is the schema-interpretation anti-pattern).

### D31. Mechanical conversions are generated; only `adapter` obligations are hooked

`drop`, `reject`, `reject-if-present`, `default` and `project` are applied by generated
field logic using the obligation metadata (ordinals, names, defaults) already computed — no
hand-written code per entry. Only `adapter` obligations (121 entries, 34 hooks) call into
`rtd-layerhooks`. This keeps the hand-written surface to the audited, irreducible logic.

### D32. The coverage gate is generation-time and bidirectional

Generation fails if a policy obligation has no implemented hook, and if a hook is not
referenced by any obligation (stale). The obligation key is the domain-separated digest
already used for dedup, so the gate is exact and survives re-imports. This mirrors the
existing "unreviewed difference / stale policy entry" enforcement and extends it to the
hook implementations.

### D33. The layer set stays manifest-driven; enabling layers is config, not code

`enabled_layers` in `schema/rtd.json` selects the set; the generator derives profiles and
the route product from it, so adding 225–231 requires no generator source change. The
canonical layer remains 232.

### D34. The source budget is raised deliberately, with a recorded reason

Enabling seven more profiles multiplies the route product and adds bodies, so the declared
sidecar size/line budget grows. The raise is recorded with the measured size and the reason
(a larger reviewed layer set), and determinism/atomicity are unchanged.

### D35. Fail-closed semantics are preserved everywhere

Admission, encoding and decoding succeed only when a body/route exists and a required hook
resolves; unavailable routes and unknown hooks fail closed with explicit errors, never a
canonical fallback. This keeps the property the server runtime and `gramsrv` rely on.

### D36. A shared hook-signature registry bridges the generator and the hooks

The adapter hooks have concrete field-level signatures, exactly as in `td`'s
`tlprofile/adapters.go` (for example a keyboard-button hook takes the profile plus the
legacy field set and returns the canonical value, or the reverse). A single generic trait
therefore does not fit. A hand-maintained registry maps each hook name to its parameters,
returns and direction; the generator consumes it to emit calls with concrete argument
types, and `rtd-layerhooks` implements exactly those functions. Generation fails when a
coverable obligation has no signature or a signature is unused, extending the existing
obligation coverage gate.

Alternatives: one generic hook trait over `Object` (rejected — the conversions are
field-structured and per-type, and a generic trait loses the compile-time field typing that
makes the ports faithful); generate the hooks from the policy note (rejected — the notes
describe the difference, not the conversion logic).

### D37. `ProfileOnly` constructors are deferred to a follow-up change

A `ProfileOnly` definition exists only in an older layer; canonical 232 has no variant for
it — `keyboardButtonRequestPhone` became `keyboardButton` plus `buttonTypeRequestPhone`. A
route on such a definition has no canonical value to convert from or to, so it needs a
generated per-profile legacy field codec plus a field-level hook, with no canonical slot.
That is a distinct and large surface (73 of 249 policy entries and 17 of the 34 hooks), so
it is deferred to `add-legacy-layer-constructors`; this change treats `ProfileOnly` as
unavailable (fail closed). Splitting keeps each change self-contained, reviewable and
CI-green; the earlier single change would have mixed two independent mechanisms.

### D38. Policy bodies are isolated per `(container-or-variant, layer)`

A container's profile codec keeps a small stack frame: its `Policy` arm is only a
`match profile.layer` that calls one generated `encode_policy_*` / `decode_policy_*`
function per supported layer, and an enum container dispatches by `route.semantic.0`
then layer. Inlining the policy bodies into the shared container codecs bloated the
`Update`-sized functions enough that even *rewrite* routes overflowed the default 2 MB
stack, so isolation is mandatory, not stylistic. Each body binds only the canonical
fields it reads (`..` elided otherwise) and names its `profile`/`_profile` parameter
according to need, so no unused-binding or unused-parameter allowance is required.

### D39. Encode recomputes the layer's flags word; decode recomputes canonical's

Encoding a canonical value into an older layer writes the layer's flags word recomputed
from the canonical value's presence bits (bit numbers from the *profile* field's
condition, presence from the canonical binding's shape). Decoding reads the layer's
fields and rebuilds the canonical value, recomputing the canonical flags word from the
reconstructed profile fields' shape, so the layer's bit layout never leaks into the
canonical value.

### D40. `default` and `reject-if-present` are generated, not hooked

`default` substitutes a recorded scalar default when a mandatory target field's source is
absent (canonical optional -> profile mandatory on encode; canonical mandatory absent from
the profile on decode), and `reject-if-present` fails closed only when a marked field is
present. Both are generated field logic; a non-scalar default is a generation error
because no default can be materialised.

### D41. Non-field hook shapes land as they are emitted, not all at once

The reviewed policy resolves some present-in-both differences to hooks whose call
site is not a single field. They are emitted incrementally, and any layer whose
obligation shape is not yet emitted stays fail-closed (never a canonical fallback):

- **field conversion** (`incompatible`, e.g. the bot-verification text family) — emitted:
  the registry's `field_convert` names the two functions, and the emitter wraps the
  surrounding `Option` and boxes the canonical class/object/method result.
- **field requirement** (`required`, e.g. the ephemeral peers) — emitted: `field_require`
  names the function that returns the value or fails when the mandatory profile field's
  optional canonical source is absent.
- **supply** (a mandatory canonical field absent from the profile whose value comes from
  a hook, e.g. the legacy default keyboard-button type), **discard/capture** (the chat
  invite query id, captured from a profile-only `webview` field), **project** (conditional
  element/field drop) and the **vector row adapter** (`layerAdaptLegacyInlineKeyboardRows`,
  which transitively needs the `ProfileOnly` keyboard buttons) are not emitted yet and are
  tracked with tasks 3.3/4.1/5.1/5.3/5.4; the two keyboard-entangled ones belong with
  `add-legacy-layer-constructors`.

This keeps the change shippable and CI-green after each shape while preserving fail-closed
semantics for the rest.

## Risks / Trade-offs

- [Very large generated sidecar hurts compile time] → Deduplicate bodies by digest, keep
  the bulk in `rtd-tlprofile`/`rtd-layerhooks`, and rely on existing build hygiene; note
  that within-crate parallelism needs a nightly `-Zthreads`.
- [A hand-written hook diverges from the reviewed `td` behavior] → The policy key/digest
  gate admits only reviewed differences; port each hook's behavior from the reference and
  pin it with a per-hook test.
- [Budget raised too far hides bloat] → Record the measured size and the reason in the
  change; keep dedup so the raise reflects real coverage, not duplication.
- [A conversion silently passes a value through] → Unknown hooks and unrepresentable
  values fail closed; tests cover the mechanical classes and the unavailable case.
- [Determinism breaks with a larger set] → Keep the same ordering and digest-based dedup;
  the two-run byte-identical test remains.

## Open Questions

- Whether to land all 34 adapter hooks in this change or split them into a follow-up if the
  hand-written surface proves large — a planning choice that does not change the specs.
