# rtd — next-session handoff

## Project

`rtd` (repo `C:\Users\roon9e\Documents\github\rtd`) is a from-scratch Rust reimplementation
of the server-facing core of the Go library `td` plus a multi-datacenter MTProto 2.0 server
runtime. Wire-compatible with real clients, Go `td`, and the official
`TelegramMessenger/mtproto-engine` (read-only GPL peer; run its `mtproto-testserver.exe` as
a test peer only). Edition 2024, MSRV 1.99, `unsafe` forbidden, clippy `all=deny`; **no
`allow(...)` anywhere**. Branch model: **main only**, pushed directly; CI runs on `main`
(`gh run list`, `gh run view <id> --log-failed`).

Active OpenSpec change: **`complete-multi-layer`** — `openspec/changes/complete-multi-layer/`.

## State

`main == origin/main` at **`7f8c367`**; CI green for every verified push. Sessions since
`b5cb9c1` implemented: mechanical policy routes, `default`, the shared hook registry + gate,
the present-in-both required/result/discard hooks, field-shaped hook call-site emission
(conversion and requirement shapes), and admission of rewrite/policy methods. Sidecar:
**12.52 MB / 381,823 lines** (16 MiB / 400k budget). Policy coverage: **165 of 179** routes
execute (mechanical + `default` + the bot text field **conversion** hooks + the ephemeral
**requirement** hooks); 14 stay fail-closed.

### Tasks done
1.1, 1.2, 2.1, 2.2, 2.3, 3.1, 3.2, 3.4, 3.5, 5.2, 6.1, 6.2, 7.2, 7.3.

### Tasks open
- **3.3** the `supply`, `discard`/capture (chat invite), `project`, and vector-row hook call
  sites (conversion and requirement shapes are done; the rest fail closed — see design D41).
- **4.1** `project` mechanical classes.
- **5.1** `layerAdaptLegacyInlineKeyboardRows` (marked `Deferred`; needs ProfileOnly).
- **5.3** the two `project` hooks.
- **5.4** coverage (blocked on 5.1/5.3).

## The linchpin: `emit_profile.rs` policy emission

`crates/rtd-codegen/src/emit_profile.rs`:

- `collect_policy_layers(universe, plan, classes, bare_ids, definition)` returns the
  supported `PolicyLayer`s. A layer is supported when every body obligation is `drop`,
  `reject-if-present`, `default`, or an `adapter` with a **field-shaped** hook
  (`rtd_layerhooks_registry::field_convert`); otherwise it fails closed.
- `collect_field_hooks(...)` validates a field conversion: both directions present, both
  sides plain value fields, **aligned optionality**, and computes the profile field's inner
  `RustType`.
- Encode (`emit_policy_layer_body`): a converted field emits
  `let __converted = <encode_fn>(binding)?;` then writes the profile inner type.
- Decode (`emit_policy_decode_fns`): reads the **profile** inner type, then shadows the
  binding with the canonical value via `<decode_fn>` and `wrap_canonical` (boxes
  class/object/method fields).
- `default`: a mandatory profile field whose canonical source is optional writes a scalar
  default (`default_expr`); a canonical mandatory field absent from the profile is rebuilt
  from `canonical_defaults` on decode.
- Hook errors flow through `ProjectionError::Hook` / `AdmitError::Hook`.

## Remaining hook shapes to emit (task 3.3 / 5.3 / 4.1)

Every present-in-both hook-backed obligation must have a call site; the registry
(`rtd-layerhooks-registry`) is the manifest. Remaining, with suggested emission:

1. **require** (`layerRequireLegacyEphemeral{Message,Send,Delete}Peer`, c2p): mandatory
   profile field, optional canonical source. Reshape the `ephemeral.rs` hooks to return the
   peer (`fn require<'a>(present: bool, peer: Option<&'a Peer>) -> Result<&'a Peer>`) and
   add `registry::field_require(name) -> Option<&str>`; encode emits
   `let __v = <fn>(binding.is_some(), binding.as_deref())?; <write __v>`. Decode already
   wraps `Some(binding)` for this shape.
2. **supply** (`layerDefaultLegacyKeyboardButtonType`, p2c): canonical mandatory field absent
   from the profile; value comes from a hook (like `canonical_defaults` but hooked).
3. **chat invite** (`layerCaptureChatInviteJoinQueryIDFromWebView` discard +
   `layerRequireCapturedChatInviteJoinQueryID` required, p2c): the profile carries a
   `webview: WebViewResult` field the canonical lacks, and the canonical `query_id` is
   captured from it. Special emission.
4. **project** (`layerProjectEphemeralBotCommand`, `layerProjectLegacyDefaultKeyboardButton`,
   c2p): conditional drop. `layerProjectEphemeralBotCommand` drops an ephemeral command from
   a vector; `layerProjectLegacyDefaultKeyboardButton` drops a default-typed button's `type`.
   NOTE: `design.md` D31 says these are generated logic, but `validate_resolution` requires a
   hook, so they are hook-backed; reconcile the prose or the policy.
5. **rows** (`layerAdaptLegacyInlineKeyboardRows`, both): converting
   `Vector<KeyboardButtonRow>` <-> `Vector<KeyboardInlineButtonRow>` transitively needs the
   `ProfileOnly` keyboard-button constructors, so it is deferred to
   `add-legacy-layer-constructors` (or implemented fail-closed on ProfileOnly buttons).

## Follow-ups after this change

- Archive `complete-multi-layer` (`openspec-archive-change`) once 3.3/4.1/5.1/5.3/5.4 close.
- **`add-legacy-layer-constructors`**: `ProfileOnly` constructors (`keyboardButtonRequestPhone`
  etc.), the 17 `old-only` hooks, the rows hook, and removing the profile-only fail-closed
  clause.
- **Multi-DC** as its own change (gramsrv v2 is the Edge/Core/Egress/File microservice split,
  not multiple DCs).
- **Client library** — to start once the server plan is complete.

## Build / verify / gotchas

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked         # includes the slow codegen generator tests
cargo run -p rtd-codegen -- verify
```
- `generate` rewrites `crates/rtd-{tg,tlprofile,mt}/src/lib.rs`; never hand-edit them.
- `rtd-tlprofile` now depends on `rtd-layerhooks`; `rtd-codegen` on `rtd-layerhooks-registry`.
- A new generated enum variant (e.g. `AdmitError::Hook`) requires updating hand-written
  exhaustive matches (`rtd-server/src/rpc.rs`) — CI caught this once.
- `rtd-tg` has TL types named `String`/`Bytes`; qualify `rtd_tg::X`. Zero-field structs use
  `_value`. A variant field may be named `value`, so the enum codec parameter is `__value`.
- Policy bodies stay isolated per `(container-or-variant, layer)` (inlining overflowed the
  stack). Decode flags recompute uses the reconstructed profile field's shape.
- PowerShell reports `Exited with code 1` even on success; trust `$LASTEXITCODE`.
