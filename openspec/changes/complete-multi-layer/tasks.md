# Tasks

## 1. Enable layers and baseline

- [x] 1.1 Set `schema/rtd.json` `enabled_layers` to 225–232 and run `cargo run -p rtd-codegen -- generate`; verify generation now reports a non-empty route product across the enabled set with nonzero body plans, and record the per-mode route counts for reference (routes=19845, body_plans=1037, preflight=1148, result=968, 3.43 MB / 125760 lines)
- [x] 1.2 Confirm the manifest already carries tdesktop provenance for layers 225–231 and that SHA-256 verification passes for every enabled layer; verify `cargo run -p rtd-codegen -- verify` reports the enabled set and succeeds

## 2. Generator — emit non-direct route bodies

- [ ] 2.1 Emit a body for every non-direct route from its conversion plan (retag/rewrite inline, profile-only as-is, policy via the hook registry, unavailable emits nothing), deduplicated by plan digest; verify a unit test per mode and a test that identical bodies share one plan
- [ ] 2.2 Emit the hook registry table (obligation key → hook) and the coverage gate that fails generation on a missing or stale hook; verify a fixture with an unimplemented hook fails and a stale hook fails
- [ ] 2.3 Raise the declared profile-sidecar size/line budget to fit the enabled set, recording the measured size/lines and the reason; verify `generate` succeeds within budget and `verify` passes
- [ ] 2.4 Verify deterministic and atomic output: two consecutive generations are byte-identical, and a simulated failure leaves prior output intact

## 3. `rtd-layerhooks` crate and hook interface

- [ ] 3.1 Add the `rtd-layerhooks` crate (depends on `rtd-tg`/`rtd-mt`) with the hook interface (canonical value + obligation context → converted value or a fail-closed error) and a registry keyed by obligation key; verify it compiles and a pass-through hook round-trips
- [ ] 3.2 Wire `rtd-tlprofile` to depend on `rtd-layerhooks` and call it for policy routes; verify a generated policy route invokes the registry and an unknown key fails closed

## 4. Mechanical conversions

- [ ] 4.1 Apply the mechanical classes (`drop`, `reject`, `reject-if-present`, `default`, `project`) from generated field logic using the obligation metadata; verify one test per class including reject-if-present absent-versus-present

## 5. Adapter hooks

- [ ] 5.1 Implement the legacy keyboard adaptation hooks (`layerAdaptLegacyKeyboard*`, `layerAdaptLegacyInputKeyboard*`, `layerAdaptLegacyInlineKeyboard*`, `layerDefaultLegacyKeyboard*`, `layerProjectLegacyDefaultKeyboard*`) with a test per hook
- [ ] 5.2 Implement the chat-invite-join hooks (`layerAdaptChatInviteJoin*`, `layerCaptureChatInviteJoinQueryIDFromWebView`, `layerRequireCapturedChatInviteJoinQueryID`) with a test per hook
- [ ] 5.3 Implement the bot-verification and ephemeral/projection hooks (`layerBotsSetCustomVerificationText`, `layerBotVerificationText`, `layerBotVerifierSettingsText`, `layerAdaptBotGuestChatResult`, `layerAdaptWebBrowserSettingsExceptionResult`, `layerProjectEphemeralBotCommand`, `layerRequireLegacyEphemeral*`) with a test per hook
- [ ] 5.4 Verify every adapter hook is referenced by the registry and none is unused (the coverage gate passes for the full enabled set)

## 6. Runtime execution and fan-out

- [ ] 6.1 Update admission to accept non-direct routes and fail closed only for unavailable routes; verify admission on one retag, rewrite, policy and profile-only route and refusal on an unavailable route
- [ ] 6.2 Update per-profile encode and decode to execute the emitted bodies; verify encode/decode round-trips on every enabled layer
- [ ] 6.3 Verify multi-profile fan-out: freeze one canonical value once and encode it for each enabled layer, confirming each output is valid for its layer

## 7. Integration verification

- [ ] 7.1 Add per-layer round-trip tests across all enabled layers and a route-completeness test asserting an explicit action for every `(wire × profile)` pair
- [ ] 7.2 Verify fail-closed coverage: unsupported layer refused, unavailable route refused, unknown hook refused, unrepresentable value refused, and no canonical fallback emitted
- [ ] 7.3 Run the full workspace gate (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and the generation-freshness check; confirm all are green

## Workflow follow-up

- Archive this change after the project's review requirements are satisfied, then verify the archived result.
- If the hand-written adapter-hook surface proves large, split the remaining hooks into a follow-up change rather than weakening the coverage gate.
- Multi-DC (DC registry, routing, migration, authorization transfer, delivery) follows as its own change; the v2 re-baseline showed gramsrv v2 is the Edge/Core/Egress/File microservice split, not multiple Telegram DCs, so multi-DC remains rtd's own design.
