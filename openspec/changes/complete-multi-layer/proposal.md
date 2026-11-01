# Proposal

## Why

rtd's multi-layer machinery is proven only on the canonical layer. All profiles 225–232 are
now enabled, but the generator computes conversion plans and the full `(wire × profile)`
classification without emitting them: `emit_profile` ignores `ConversionPlan`/`PlanSet`, and
the runtime returns `RouteNotEmitted` for any non-direct route. So the five non-direct modes
are inert and older-layer traffic fails closed.

This change makes the routes whose definition exists in **both** canonical 232 and the
profile (`Present` families) executable: retag, rewrite, and the policy/mechanical
conversions among them. Routes whose definition exists **only** in an older layer
(`ProfileOnly`, e.g. the legacy `keyboardButtonRequestPhone`) have no canonical value to
convert from; they need generated per-profile legacy field codecs and are deferred to a
follow-up change (see `design.md` D37), so this change stays self-contained and reviewable.

## What Changes

- **Generator emits `Present` non-direct route bodies** from the already-computed
  `ConversionPlan`/`PlanSet`: retag (profile id + canonical body), rewrite (a per-profile
  field-mapped encoder/decoder that recurses into dirty sub-families), and policy (the same
  plus mechanical classes and hook calls). Bodies are deduplicated by plan digest.
- **Shared hook-signature registry**: the hooks have concrete field-level signatures (as in
  `td`), so a hand-maintained registry maps each hook name to its parameters, returns and
  direction. Both the generator (to emit calls) and the hand-written `rtd-layerhooks` crate
  (to implement them) consume it, and generation fails when a present-in-both obligation is
  not covered or a hook is stale.
- **Reviewed conversion policy for `Present` families is implemented**: the mechanical
  classes (`drop`, `reject`, `reject-if-present`, `default`, `project`) applied from
  generated field logic, and the adapter hooks for `incompatible`, `required`, `result`,
  `field-projection` and `discard` obligations.
- **Runtime executes `Present` non-direct routes**: admission, per-profile encode and decode
  handle retag/rewrite/policy; `Unavailable` still fails closed (absence is the action), and
  a `ProfileOnly` route fails closed until the follow-up lands.
- **Multi-profile fan-out**: freeze a canonical value once and encode it for any enabled
  layer whose route is `Present`.
- **Source budget**: the sidecar grows; the declared budget is raised deliberately and
  recorded; generation stays deterministic and atomic.

## Capabilities

### New Capabilities

- `layer-conversion-policy`: the reviewed per-layer conversion behavior for families present
  in both canonical and profile — the mechanical classes and adapter hooks — that makes
  `Present` non-direct routes execute exactly as audited, plus the coverage gate that keeps
  every such obligation implemented.

### Modified Capabilities

- `tl-codegen`: the generator emits `Present` non-direct route bodies and the hook-signature
  registry, and fails when a coverable obligation is not implemented or a hook is stale.
- `multi-layer-tl`: `Present` non-direct routes are admitted and executed; the system fails
  closed only for genuinely unavailable routes (and, for now, `ProfileOnly`).

## Impact

- **Crates**: `rtd-codegen` gains body/registry emission and a hook-signature model; a new
  hand-written `rtd-layerhooks` crate holds the `Present` adapter hooks (the generated
  `rtd-tlprofile` stays never-hand-edited and calls into it); `rtd-tlprofile` grows and
  depends on it; `rtd-server` is unchanged (it already dispatches through `rtd-tlprofile`).
- **Schema/config**: `schema/rtd.json` already enables 225–232; the canonical model is
  unchanged (canonical layer stays 232); provenance is unchanged.
- **Budget**: the profile sidecar's declared size/line budget is raised with a recorded
  reason.
- **Compatibility**: layer-232 wire behavior is unchanged; `Present` non-direct traffic on
  layers 225–231 becomes served instead of refused; `ProfileOnly` constructors are still
  refused until the follow-up change.
