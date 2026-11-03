# Tasks

## 1. Resolvers

- [x] 1.1 Add `Resolver`/`Endpoint`/`Protocol` and default DC lists (`dcs/mod.rs`)
- [ ] 1.2 Implement Plain and MTProxy resolvers; wire WebSocket resolver
- [ ] 1.3 Implement the DNS resolver (decode the encrypted config against vendored keys)

## 2. Migration

- [ ] 2.1 Handle `_MIGRATE_*` errors at the invoke boundary and switch the pool target (`client.rs`)
- [ ] 2.2 Implement authorization export/import across DCs

## 3. Verification

- [ ] 3.1 Verify a `PHONE_MIGRATE_2` response switches the target DC and the call succeeds (`tests/dcs.rs`)
- [ ] 3.2 Verify DNS config decode against a known vector
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
