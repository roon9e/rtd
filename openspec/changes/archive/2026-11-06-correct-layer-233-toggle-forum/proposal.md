# Proposal

## Why

The layer-233 work was corrected upstream (merged as `iamxvbaba/td#5`). Instead of **rejecting** a
historical `channels.toggleForum` request that cannot express the new required `sidebar` field,
the reviewed decision is to **admit it and default `sidebar` to false** — the historical wire
already distinguishes the list and tabs layouts, and an old client must not be locked out.

The schema and all other decisions are unchanged; only the eight `channels.toggleForum`
required-field obligations (layers 225–232, profile→canonical) change from `reject` to `default`.

## What Changes

- Adopt the corrected `td` `policy.json` (same 305 keys; 8 resolutions `reject` → `default`).
- Regenerate `rtd-tlprofile`; add a runtime test for the defaulting behavior.

## Capabilities

### Modified Capabilities

- `multi-layer-tl`: required-field route defaulting for `channels.toggleForum`.

## Impact

- **Crates**: `rtd-tlprofile` (generated + tests), `schema/layers/policy.json`.
- **Non-goals**: no schema change; no other policy entry changes.
