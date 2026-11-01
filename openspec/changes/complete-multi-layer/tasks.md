# Tasks

## 1. Enable layers and baseline

- [x] 1.1 Set `schema/rtd.json` `enabled_layers` to 225–232 and run `cargo run -p rtd-codegen -- generate`; verify generation now reports a non-empty route product across the enabled set with nonzero body plans, and record the per-mode route counts for reference (routes=19845, body_plans=1037, preflight=1148, result=968, 3.43 MB / 125760 lines)
- [x] 1.2 Confirm the manifest already carries tdesktop provenance for layers 225–231 and that SHA-256 verification passes for every enabled layer; verify `cargo run -p rtd-codegen -- verify` reports the enabled set and succeeds

## 2. Hook-signature registry and `rtd-layerhooks` skeleton

- [x] 2.1 Define the shared hook-signature registry (name to parameters, returns and direction) for the present-in-both adapter hooks, consuming both the generator and the hooks crate; verify the generator looks up every present-in-both adapter obligation's signature and fails on a missing one (new dependency-free `rtd-layerhooks-registry` crate; `rtd-codegen::check_hook_registry` gates direction coverage and staleness, and tests assert a missing/direction-mismatched/stale signature fails; `rtd-layerhooks` asserts its `IMPLEMENTED_HOOKS` equals the registry's implemented set)
- [x] 2.2 Add the `rtd-layerhooks` crate (depends on `rtd-tg`/`rtd-mt`) with a registry type and the field-level hook function shapes; verify it compiles and `cargo clippy -p rtd-layerhooks --all-targets -- -D warnings` is clean
- [x] 2.3 Wire the coverage gate so generation fails when a present-in-both adapter obligation has no implemented hook or a hook is unused (stale); verify both failures with fixtures (`check_hook_registry` fails on a missing signature, a direction mismatch, an unimplemented (`Pending`) hook and a stale entry; fixtures assert each; the one `Deferred` hook, `layerAdaptLegacyInlineKeyboardRows`, is documented as depending on the ProfileOnly follow-up)

## 3. Generator — emit present-in-both non-direct bodies

- [x] 3.1 Emit a body for every present-in-both non-direct route from its conversion plan, deduplicated by digest; verify a unit test per emitted mode and a test that identical bodies share one plan
- [x] 3.2 Emit retag as the profile wire id plus the canonical body, and rewrite as a per-profile field-mapped codec recursing into dirty sub-families; verify a rewrite round-trip on a present-in-both family
- [ ] 3.3 Emit policy call sites using the registry's concrete signatures for present-in-both obligations; verify a policy route invokes its hook
- [x] 3.4 Raise the declared profile-sidecar size/line budget to fit the emitted bodies, recording the measured size and the reason; verify `generate` succeeds within budget and `verify` passes (measured 12,455,790 bytes / 379,876 lines; the mechanical per-`(container-or-variant, layer)` policy bodies are isolated into their own functions rather than inlined, which keeps container frames small and fits the existing 16 MiB / 400k budget, so no raise was required; `generate` and `verify` both pass)
- [x] 3.5 Verify deterministic and atomic output: two consecutive generations are byte-identical (`generate.rs::generation_is_deterministic`), and a simulated failure leaves prior output intact (`output.rs::write_atomic_failure_leaves_prior_output_intact`, plus commit/no-staging and `verify` fresh/stale/missing tests)

## 4. Mechanical conversions

- [ ] 4.1 Apply the mechanical classes (`drop`, `reject`, `reject-if-present`, `default`, `project`) from generated field logic; verify one test per class including reject-if-present absent-versus-present

## 5. Present-in-both adapter hooks

- [ ] 5.1 Implement the `incompatible` adapter hooks; verify a test per hook (the bot-verification text family — `layerBotVerificationText`, `layerBotVerifierSettingsText`, `layerBotsSetCustomVerificationText` — is implemented with tests; `layerAdaptLegacyInlineKeyboardRows` is `Deferred` because the row buttons transitively need the ProfileOnly keyboard-button constructors)
- [x] 5.2 Implement the `required` and `result` adapter hooks; verify a test per hook (`required`: `layerRequireLegacyEphemeral{Message,Send,Delete}Peer`, `layerRequireCapturedChatInviteJoinQueryID`, `layerDefaultLegacyKeyboardButtonType`; `result`: `layerAdaptBotGuestChatResult`, `layerAdapt{UpdatesToChatInviteJoinResult,ChatInviteJoinResultToUpdates,WebBrowserSettingsExceptionResult}` — each has a unit test including the fail-closed case)
- [ ] 5.3 Implement the `field-projection` and `discard` adapter hooks; verify a test per hook (`layerCaptureChatInviteJoinQueryIDFromWebView` (discard) is implemented with tests; the `project` hooks `layerProjectEphemeralBotCommand` and `layerProjectLegacyDefaultKeyboardButton` need the policy call-site emission before their exact signatures are fixed, so they are registered as `Deferred` and land with task 3.3)
- [ ] 5.4 Verify coverage: every present-in-both adapter obligation is implemented and no present-in-both hook is unused

## 6. Runtime execution and fan-out

- [ ] 6.1 Update admission, per-profile encode and decode to execute present-in-both non-direct bodies; verify admission and encoding on a retag, a rewrite and a policy route, and fail-closed on an unavailable and a profile-only route
- [ ] 6.2 Verify multi-profile fan-out: freeze one canonical value and encode it for each enabled layer with a present-in-both route, confirming each output is valid for its layer

## 7. Integration verification

- [ ] 7.1 Add per-layer round-trip tests for present-in-both families across the enabled layers and a route-completeness test asserting an explicit action for every `(wire × profile)` pair
- [ ] 7.2 Verify fail-closed coverage: unsupported layer refused, unavailable route refused, profile-only route refused, unknown hook refused, unrepresentable value refused, and no canonical fallback emitted (covered now: unsupported layer (`unsupported_layer_is_refused`), unrepresentable value (`unrepresentable_value_fails_closed`), profile-only route (`profile_only_route_is_refused`) and an unrouted wire id (`unknown_wire_id_is_refused`); unknown-hook and no-canonical-fallback await the hook call-site emission)
- [ ] 7.3 Run the full workspace gate (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and the generation-freshness check; confirm all are green

## Workflow follow-up

- Archive this change after the project's review requirements are satisfied, then verify the archived result.
- `add-legacy-layer-constructors` (the `ProfileOnly` follow-up) implements the per-profile legacy field codecs and the 17 `old-only` hooks, and removes the profile-only fail-closed clause.
- Multi-DC (DC registry, routing, migration, authorization transfer, delivery) follows as its own change; the v2 re-baseline showed gramsrv v2 is the Edge/Core/Egress/File microservice split, not multiple Telegram DCs, so multi-DC remains rtd's own design.
