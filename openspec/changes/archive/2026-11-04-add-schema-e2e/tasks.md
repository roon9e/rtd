# Tasks

## 1. Import

- [x] 1.1 Import `_schema/encrypted.tl` and `_schema/trace.tl` with provenance (`rtd-codegen import`)

## 2. Emit

- [x] 2.1 Emit `rtd-tg-e2e` (secret chats: `DecryptedMessage`, media, actions, selectors)
- [x] 2.2 Emit `rtd-tgtrace` (hand-maintained: the generic `invokeWithTrace` wrapper is defined over the canonical `rtd_tg::Method` and is not expressible by the standalone value emitter)
- [x] 2.3 Wire both into the workspace and `rtd-codegen verify`

## 3. Verification

- [x] 3.1 Verify generation freshness produces no diff
- [x] 3.2 Verify a secret-chat message and a trace object round-trip (generated tests)
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
