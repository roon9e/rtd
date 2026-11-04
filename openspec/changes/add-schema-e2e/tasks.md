# Tasks

## 1. Import

- [ ] 1.1 Import `_schema/encrypted.tl` and `_schema/trace.tl` with provenance (`rtd-codegen import`)

## 2. Emit

- [ ] 2.1 Emit `rtd-tg-e2e` (secret chats: `DecryptedMessage`, media, actions, selectors)
- [ ] 2.2 Emit `rtd-tgtrace`
- [ ] 2.3 Wire both into the workspace and `rtd-codegen verify`

## 3. Verification

- [ ] 3.1 Verify generation freshness produces no diff
- [ ] 3.2 Verify a secret-chat message and a trace object round-trip (generated tests)
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
