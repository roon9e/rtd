# Proposal

## Why

rtd's multi-layer machinery is proven only on the canonical layer. Exactly one profile
(232) is enabled, every enabled route is `direct`, and although the generator computes
conversion plans and the full `(wire × profile)` route classification, it does not emit
them: `emit_profile` ignores `ConversionPlan`/`PlanSet`, and the runtime returns
`RouteNotEmitted` for any non-direct route. So older clients (layers 225–231) are refused,
and five of the six route modes (retag, rewrite, policy, profile-only, unavailable) are
inert. Stage 2 of the foundation's migration plan is what makes rtd genuinely multi-layer:
layers 225–231 admitted **exactly**, non-direct routes executed, and the reviewed
conversion policy implemented — all fail-closed and within the generated source budget.

## What Changes

- **Enable layers 225–231** alongside canonical 232 (`schema/rtd.json`); the manifest
  already holds the complete reviewed set and provenance.
- **Generator emits non-direct route bodies** (retag/rewrite/policy/profile-only) from the
  already-computed `ConversionPlan`/`PlanSet`, replacing the `RouteNotEmitted` stub, and
  emits a **hook registry** for policy routes.
- **Reviewed conversion policy is implemented**: the 249 policy entries (34 distinct
  adapter hooks — legacy keyboard adaptation, chat-invite-join result adaptation,
  bot-verification text, ephemeral projection — plus mechanical drop / reject /
  reject-if-present / default / project) become executable, referenced by semantic key.
- **Runtime executes non-direct routes**: admission, per-profile encode and decode handle
  retag/rewrite/policy/profile-only; it fails closed only for genuinely `unavailable`
  routes (absence is the action).
- **Multi-profile fan-out**: freeze a canonical value once and encode it for any enabled
  layer, so sessions on 225–231 receive per-profile results and updates.
- **Source budget**: the sidecar grows substantially; the declared budget is raised
  deliberately and recorded, and generation stays deterministic and atomic.

## Capabilities

### New Capabilities

- `layer-conversion-policy`: the reviewed per-layer conversion behavior — the policy hooks
  and their mechanical counterparts — that makes non-direct routes execute exactly as
  audited, and the coverage gate that keeps every obligation implemented.

### Modified Capabilities

- `tl-codegen`: the generator emits non-direct route bodies and the hook registry, and
  fails when a policy obligation is not covered by an implemented hook or a hook is stale.
- `multi-layer-tl`: non-direct routes are admitted and executed; the system fails closed
  only for genuinely unavailable routes rather than for every non-direct route.

## Impact

- **Crates**: `rtd-codegen` gains body and hook emission; a new hand-written crate
  `rtd-layerhooks` holds the conversion hooks (so the generated `rtd-tlprofile` stays
  never-hand-edited and calls into it); `rtd-tlprofile` grows and depends on
  `rtd-layerhooks`; `rtd-server` needs no change (it already dispatches through
  `rtd-tlprofile`).
- **Schema/config**: `schema/rtd.json` enables 225–231; `rtd-tg` canonical model is
  unchanged (canonical layer stays 232); provenance is unchanged.
- **Budget**: the profile sidecar's declared size/line budget is raised with a recorded
  reason.
- **Compatibility**: wire behavior for existing layer-232 sessions is unchanged; older
  layers become supported instead of refused.
