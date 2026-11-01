# rtd — next-session handoff

## Project

`rtd` (repo `C:\Users\roon9e\Documents\github\rtd`, GitHub `roon9e/rtd`) is a from-scratch
Rust reimplementation of the server-facing core of the Go library `td`
(`C:\Users\roon9e\Documents\github\td`) plus a multi-datacenter MTProto 2.0 server
runtime. Wire-compatible with real clients, Go `td`, and the official
`TelegramMessenger/mtproto-engine` (read-only GPL peer; run its
`mtproto-testserver.exe` as a test peer only). Edition 2024, MSRV 1.99, `unsafe` forbidden,
clippy `all=deny`; **no `allow(...)` anywhere** (hand-written or generated). Branch model:
**main only**, pushed directly; CI runs on `main` (watch with `gh run list` / `gh run view`).

Active OpenSpec change: **`complete-multi-layer`** — read
`openspec/changes/complete-multi-layer/{proposal,design,tasks}.md` and `specs/`.

## Git / CI state

- `main == origin/main` at **`bded7f5`**, CI green for the earlier commits; the last three
  pushes were still running when this was written (check `gh run list`).
- This session pushed four commits:
  - `f8ee2d8` mechanical policy routes end-to-end (142/179 policy routes).
  - `a10bcb4` output atomicity/freshness tests (task 3.5).
  - `faa9fec` shared hook registry + generator lookup gate (task 2.1).
  - `9ec8b6a` present-in-both required/result/discard hooks + strict gate (tasks 2.3, 5.2).
  - `bded7f5` gate the project-action hooks too.
- Tree is clean. Regenerating is unchanged: `profile_bytes=12455790 profile_lines=379876`,
  `routes=19845 direct=18489 retag=4 rewrite=1100 unavailable=0 policy=179 profile_only=73`.

## Done in `complete-multi-layer`

- **1.1, 1.2, 2.2, 3.1, 3.2, 3.4, 3.5** and now **2.1, 2.3, 5.2**.
- **Policy (mechanical)**: `emit_profile.rs` emits isolated per-`(container-or-variant,
  layer)` functions for present-in-both families whose every body obligation is `drop` or
  `reject-if-present`. Encode recomputes the layer flags word; decode rebuilds the canonical
  value and recomputes the canonical flags word. This covers **142 of 179** policy routes;
  the rest (adapter/project/default) stay fail-closed.
- **Hook registry** `crates/rtd-layerhooks-registry` (dependency-free): `HookSpec { name,
  direction, status, signature }`, `HOOKS`, `lookup`, `implemented_names`. `Status` is
  `Implemented | Pending | Deferred`.
- **Gate** `rtd-codegen::generate::check_hook_registry(plan, hooks: &[HookSpec])` runs in
  `generate`: for every present-in-both obligation whose action is `adapter` **or** `project`
  (and kind != `old-only`) it requires a registered hook whose `direction` covers the
  obligation and whose status is not `Pending`; then every registry entry must be referenced
  (stale fails). Tests cover missing / direction-mismatch / unimplemented / stale
  (`hook_registry_fails_on_*`). `rtd-layerhooks::implemented_hooks_match_the_registry`
  asserts `IMPLEMENTED_HOOKS == registry::implemented_names()`.
- **Implemented hooks** in `crates/rtd-layerhooks/src/` (each with fail-closed tests):
  - `text.rs` — bot-verification text family (3 hooks).
  - `results.rs` — `layerAdaptBotGuestChatResult`,
    `layerAdaptChatInviteJoinResultToUpdates`, `layerAdaptUpdatesToChatInviteJoinResult`,
    `layerAdaptWebBrowserSettingsExceptionResult`.
  - `ephemeral.rs` — `layerRequireLegacyEphemeral{Message,Send,Delete}Peer`.
  - `keyboard.rs` — `layerDefaultLegacyKeyboardButtonType`.
  - `chat_invite.rs` — `layerCaptureChatInviteJoinQueryIDFromWebView`,
    `layerRequireCapturedChatInviteJoinQueryID`.
- **Deferred (registered, `Status::Deferred`)**: `layerAdaptLegacyInlineKeyboardRows` (row
  buttons transitively need the ProfileOnly keyboard-button constructors) and the two
  `project` hooks `layerProjectEphemeralBotCommand` / `layerProjectLegacyDefaultKeyboardButton`
  (signatures pin with the emission below). The 17 `old-only` hooks are absent by design
  (ProfileOnly, follow-up change `add-legacy-layer-constructors`).

## Next: task 3.3 — policy call-site emission (the linchpin)

Everything remaining hangs off emitting the adapter/project hook calls into the generated
policy bodies and the result path. Steps:

1. **Enrich the registry with machine-readable structure.** Today `signature` is an
   informative string. Emission needs, per obligation/direction: the concrete function
   path (e.g. `rtd_layerhooks::results::chat_invite_join_result_to_updates`), the
   parameter list (which canonical/profile fields, in order, and their Rust types), and the
   return shape (value / tuple / `Option` "drop"). Model this in
   `rtd-layerhooks-registry` (still dependency-free; use type *strings*).
2. **Emit hook calls** in `emit_profile.rs`. Consider: an obligation is either
   *whole-value* (result hooks; `layerAdaptLegacyInlineKeyboardRows`) or *field-shaped*
   (text/ephemeral/keyboard/chat-invite). Field-shaped hooks read/write specific fields;
   the emitter must map obligation `field`/`other_field` + `FieldMapping` ordinals to those
   fields. Result hooks belong on the RPC result path, not the value codec
   (`body_obligations()` excludes `result`).
3. **`default` mechanical (task 4.1).** Needs no hook. The 5 obligations:
   `pageListOrderedItem{Text,Blocks}.num` (canonical optional -> profile mandatory, c2p) and
   `auth.sentCodePaymentRequired.premium_days` (canonical mandatory, absent in profile, p2c).
   Add `default_expr(ty) -> Option<String>` for scalar types and fail closed for others.
4. **`project` action.** `layerProjectEphemeralBotCommand` drops an ephemeral command from a
   vector; `layerProjectLegacyDefaultKeyboardButton` drops a default-typed button's `type`.
   Pin shapes with the emission, then implement and flip to `Implemented`.
5. `layerAdaptLegacyInlineKeyboardRows`: implement a fail-closed version (present-in-both
   buttons convert; a ProfileOnly button -> `HookError::Unrepresentable`) or keep `Deferred`.

### Emitter pitfalls learned (important)

- `rtd-tg` has TL types literally named `String`/`Bytes`; qualify as `rtd_tg::X`. Zero-field
  structs take `_value`. Codec names are snake_case.
- Policy bodies **must** stay isolated per `(container-or-variant, layer)`; inlining made
  rewrite routes overflow the default 2 MB stack.
- When a profile field is **mandatory** but its canonical source is **optional** (the
  `default`/`required` shape), the canonical binding is `Option<T>` while the field type is
  `T`: encode needs `if let Some(v) = b { .. } else { default }`, decode needs `Some(binding)`.
- The flags recompute must use the **profile** field's shape for the presence test
  (mandatory -> always set; `presence_only` -> `if b`; optional -> `if b.is_some()`) and the
  **canonical** field's `condition.bit` for the bit. The current code uses the canonical
  field's shape, which is only correct because drop-only layers keep the shapes aligned.
- `collect_policy_layers` currently allows actions `{"drop","reject-if-present"}`; a layer
  with any other action fails closed. `every_profile_field_mapped` and `rebuildable` gate
  which shapes are emittable.
- `fail-closed` is `ProjectionError::RouteNotEmitted` (encode) / `AdmitError::UnsupportedRoute`
  (decode); reject uses `ProjectionError::Unrepresentable { layer, wire_id }`.

## Build / verify

```
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo run -p rtd-codegen -- verify        # freshness, after `generate`
```
`generate` rewrites `crates/rtd-tg/src/lib.rs`, `crates/rtd-tlprofile/src/lib.rs`,
`crates/rtd-mt/src/lib.rs` — all committed, never hand-edit. The codegen test suite runs the
full generator several times (~15 min); prefer `cargo test -p <crate>` and let CI cover the
rest. PowerShell reports `Exited with code 1` even on success — trust `$LASTEXITCODE` and the
parsed `test result:` lines. Peer gate (local, no VPN):
`RTD_PEER_TESTSERVER=C:\Users\roon9e\Documents\github\mtproto-engine\target\release\mtproto-testserver.exe`,
then `cargo test -p rtd-exchange --test peer_gate`.
