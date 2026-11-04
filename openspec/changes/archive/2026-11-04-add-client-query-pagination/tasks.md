# Tasks

## 1. Framework

- [x] 1.1 Add `Query<T>` iterator trait and `ForEach`/`Fetch` drivers (`query/mod.rs`)
- [x] 1.2 Add a request hasher for cached queries

## 2. Queries

- [x] 2.1 Implement history, dialogs, participants, photos and blocked iterators (`pagination.rs`; featured stickers remain)
- [x] 2.2 Implement the `cached` variants (hash-based)

## 3. Verification

- [x] 3.1 Verify an iterator pages to completion and stops on a short page (`query` tests)
- [x] 3.2 Verify a cached query only requests changed items
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
