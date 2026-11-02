# Tasks

## 1. Per-profile legacy field codecs

- [ ] 1.1 Emit a per-profile value type and field codec for every `ProfileOnly` constructor the enabled layers need; verify a generated round trip for one legacy constructor on the layer that defines it (the surface is exactly **18 keyboard-button constructors** on layers 225–228 — `keyboardButton{Url,Callback,RequestPhone,RequestGeoLocation,RequestPoll,RequestPeer,SwitchInline,Game,Buy,UrlAuth,UserProfile,WebView,SimpleWebView,Copy}`, `inputKeyboardButton{RequestPeer,UrlAuth,UserProfile}`, `inputPasskeyCredentialFirebasePNV` — plus `inputPasskeyCredentialFirebasePNV` alone on 229; `cargo run -p rtd-codegen -- debug` lists them: `profile_only` lines)
- [ ] 1.2 Wire the legacy codecs into `emit_profile`'s profile codec and dispatchers, deduplicated by plan digest; verify generation stays deterministic and within a recorded budget

## 2. `old-only` adapter hooks

- [x] 2.1 Implement the 17 `old-only` hooks in `rtd-layerhooks` from `td/tlprofile/adapters.go`; verify a unit test per hook including the fail-closed case (`legacy.rs`: 6 `KeyboardButton` + 11 `KeyboardInlineButton` hook pairs, each a `-> canonical` builder and a `-> fields` extractor that fails closed with `HookError::WrongVariant`; tests cover the built type, the extracted fields and the wrong-variant path)
- [ ] 2.2 Register the emitted legacy codecs and the `old-only` hooks in `rtd-layerhooks-registry` and flip them to `Implemented`; verify the generation gate covers every `old-only` obligation and reports none stale

## 3. Row adapter

- [x] 3.1 Implement `layerAdaptLegacyInlineKeyboardRows` over the legacy button hooks; verify a row round trip and fail-closed on an unrepresentable button (`runtime.rs`: `policy_legacy_inline_keyboard_rows_round_trip` fans out across every enabled layer and `policy_legacy_inline_keyboard_rows_fail_closed_on_an_unrepresentable_button` refuses a button with no older constructor)
- [x] 3.2 Emit the row adapter call site; verify the `replyInlineMarkup` policy route on an older layer executes (`runtime.rs`: `policy_legacy_inline_keyboard_rows_uses_the_profile_row_constructor` checks the layer-225 policy route writes the legacy `keyboardButtonRow#77608b83` and `keyboardButtonUrl#d80c25ec`; `..._reject_a_reply_only_button` fails closed on a reply-only button in an inline row)

## 4. Runtime and clause removal

- [ ] 4.1 Serve `ProfileOnly` routes in admission, per-profile encode and decode with no canonical fallback; verify a `ProfileOnly` method and object round trip and fail-closed on a genuinely unrepresentable value
- [ ] 4.2 Retire the `multi-layer-tl` "profile-only fails closed" clause with a MODIFIED requirement once `complete-multi-layer` is archived

## 5. Verification

- [ ] 5.1 Add per-layer round-trip tests for the legacy constructors and a route-completeness test asserting an explicit action for every profile-only `(wire × profile)` pair
- [ ] 5.2 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace`, generation freshness) and confirm all are green
