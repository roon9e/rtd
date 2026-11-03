# Tasks

## 1. Entities

- [ ] 1.1 Add `User`/`Chat`/`Channel`/`Supergroup` with `InputPeer` and access hashes (`peers/mod.rs`)
- [x] 1.2 Add the `Storage` trait (in-memory default)

## 2. Resolution

- [x] 2.1 Implement `PeerManager` username resolution with caching
- [x] 2.2 Add the `members` submodule (iteration + rights)

## 3. Verification

- [ ] 3.1 Verify a username resolves to a cached entity with an access hash (`peers` tests)
- [ ] 3.2 Verify an update refreshes the cached entity
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
