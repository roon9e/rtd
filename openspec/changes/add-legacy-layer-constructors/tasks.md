# Tasks

## 1. Per-profile legacy field codecs

- [x] 1.1 Serve every `ProfileOnly` constructor the enabled layers need through the reviewed `old-only` hooks (the surface is exactly **18 keyboard-button constructors** on layers 225–228 — `keyboardButton{Url,Callback,RequestPhone,RequestGeoLocation,RequestPoll,RequestPeer,SwitchInline,Game,Buy,UrlAuth,UserProfile,WebView,SimpleWebView,Copy}`, `inputKeyboardButton{RequestPeer,UrlAuth,UserProfile}`, `inputPasskeyCredentialFirebasePNV` — plus `inputPasskeyCredentialFirebasePNV` alone on 229, which has no hook and stays fail-closed; instead of a separate per-profile value type, `collect_legacy_classes` resolves the older layer's field layout and the canonical class it maps to, and `emit_legacy_class_codec` emits per-layer `decode_legacy_*`/`encode_legacy_*` that read/write those fields and call the hook — design D1/D5)
- [x] 1.2 Wire the legacy dispatch into `emit_profile`'s class codecs (`RouteMode::ProfileOnly` preludes in `encode_*_profile`/`decode_*_profile`) and the `Object` decoder (`decode_object_profile` routes a profile-only wire id to its canonical class); verify a `ProfileOnly` object round trip for both a reply (`profile_only_keyboard_button_round_trips`) and an inline (`profile_only_keyboard_inline_button_round_trips`) constructor; the sidecar stays within the recorded budget (13.16 MB / 395,109 lines of 16 MiB / 400k)

## 2. `old-only` adapter hooks

- [x] 2.1 Implement the 17 `old-only` hooks in `rtd-layerhooks` from `td/tlprofile/adapters.go`; verify a unit test per hook including the fail-closed case (`legacy.rs`: 6 `KeyboardButton` + 11 `KeyboardInlineButton` hook pairs, each a `-> canonical` builder and a `-> fields` extractor that fails closed with `HookError::WrongVariant`; tests cover the built type, the extracted fields and the wrong-variant path)
- [x] 2.2 Register the `old-only` hooks and their canonical identity in `rtd-layerhooks-registry` (`LEGACY_HOOKS`, `legacy_for_wire`, `legacy_params`, `legacy_param_tuple_prefix`) and the generation gate `check_legacy_hook_registry`; verify it covers every `old-only` adapter obligation and reports none stale (the gate requires each adapter obligation to resolve to a registered hook whose name matches, treats an `old-only` `reject` obligation — the passkey credential — as hookless by design, and rejects a registered hook referenced by no obligation; note the `old-only` hooks are field-level, not the `HOOKS` `Status`, so they are not flipped to `Implemented`; the `layerAdaptLegacyInlineKeyboardRows` hook is `Status::Generated`)

## 3. Row adapter

- [x] 3.1 Implement `layerAdaptLegacyInlineKeyboardRows` over the legacy button hooks; verify a row round trip and fail-closed on an unrepresentable button (`runtime.rs`: `policy_legacy_inline_keyboard_rows_round_trip` fans out across every enabled layer and `policy_legacy_inline_keyboard_rows_fail_closed_on_an_unrepresentable_button` refuses a button with no older constructor)
- [x] 3.2 Emit the row adapter call site; verify the `replyInlineMarkup` policy route on an older layer executes (`runtime.rs`: `policy_legacy_inline_keyboard_rows_uses_the_profile_row_constructor` checks the layer-225 policy route writes the legacy `keyboardButtonRow#77608b83` and `keyboardButtonUrl#d80c25ec`; `..._reject_a_reply_only_button` fails closed on a reply-only button in an inline row)

## 4. Runtime and clause removal

- [x] 4.1 Serve `ProfileOnly` routes in per-profile encode and decode with no canonical route fallback; verify a `ProfileOnly` object round trip and fail-closed on a genuinely unrepresentable value (`profile_only_keyboard_button_round_trips`, `profile_only_keyboard_inline_button_round_trips`; `profile_only_route_without_a_hook_fails_closed` refuses `inputPasskeyCredentialFirebasePNV`; there are no `ProfileOnly` *methods*, so the skeleton-admission path is unchanged)
- [ ] 4.2 Retire the `multi-layer-tl` "profile-only fails closed" clause with a MODIFIED requirement once `complete-multi-layer` is archived

## 5. Verification

- [x] 5.1 Add per-layer round-trip tests for the legacy constructors and a route-completeness test asserting an explicit action for every profile-only `(wire × profile)` pair (the reply/inline `ProfileOnly` round trips above; `every_present_definition_has_a_route_in_its_layer` already requires an explicit route for every definition present in an enabled layer, including the profile-only constructors)
- [x] 5.2 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace`, generation freshness) and confirm all are green (all green in WSL `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked` including the codegen generation-freshness tests)
