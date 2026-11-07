# Proposal

## Why

The `td` reference gained a reviewed projection (merged as `iamxvbaba/td#6`): a Layer 233
`messageMediaWidget` has no wire identity before Layer 233, so the generic new-only policy dropped
the media and a widget-only message reached a 225–232 client as an empty text message. The
corrected policy projects it to `messageMediaUnsupported#9f84f49e` instead, so the older client
renders an explicit unsupported-media placeholder. `rtd` must track it.

## What Changes

- Adopt the corrected `td` `policy.json` (305 → 313 entries: eight `messageMediaWidget` new-only
  obligations resolved with action `project` + hook `layerProjectLegacyWidgetMedia`).
- Add the **first family-level project hook**: the generator substitutes another canonical member
  when an older profile lacks the constructor.
- Implement the hook in `rtd-layerhooks` and register its signature.

## Capabilities

### Modified Capabilities

- `multi-layer-tl`: family-level media projection.

## Impact

- **Crates**: `rtd-codegen` (generator feature), `rtd-layerhooks`, `rtd-layerhooks-registry`,
  `rtd-tlprofile` (generated + tests), `schema/layers/policy.json`.
- **Non-goals**: no schema change; no other policy entry changes.
