# Tasks

## 1. Framework

- [ ] 1.1 Add `Query<T>` iterator trait and `ForEach`/`Fetch` drivers (`query/mod.rs`)
- [ ] 1.2 Add a request hasher for cached queries

## 2. Queries

- [ ] 2.1 Implement participants, blocked, photos, dialogs, messages and featured-sticker iterators
- [ ] 2.2 Implement the `cached` variants (hash-based)

## 3. Verification

- [ ] 3.1 Verify an iterator pages to completion and stops on a short page (`query` tests)
- [ ] 3.2 Verify a cached query only requests changed items
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
