# Tasks

## 1. Pool

- [ ] 1.1 Add per-DC `Pool` with acquire/release, free list and `MaxOpenConnections` (`pool.rs`)
- [ ] 1.2 Add `SessionOptions` (DC, auth key, salt, PFS-aware key selection)

## 2. Routing

- [ ] 2.1 Route invokes through the pool, replacing dead connections and retrying once on a connection error (`client.rs`)

## 3. Verification

- [ ] 3.1 Verify a dead connection is replaced and the next call succeeds (`tests/pool.rs`)
- [ ] 3.2 Verify the open-connection cap is enforced
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
