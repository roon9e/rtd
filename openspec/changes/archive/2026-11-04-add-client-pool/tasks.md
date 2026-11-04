# Tasks

## 1. Pool

- [x] 1.1 Add a bounded `Pool` with acquire/release, idle reuse and a capacity cap (`pool.rs`)
- [x] 1.2 Add `SessionOptions` (DC, auth key, salt, PFS-aware key selection)

## 2. Routing

- [x] 2.1 Route invokes through the pool, replacing dead connections and retrying once on a connection error (`client.rs`)

## 3. Verification

- [x] 3.1 Verify a dead connection is replaced and the next call succeeds (`tests/pool.rs`)
- [x] 3.2 Verify the open-connection cap is enforced
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
