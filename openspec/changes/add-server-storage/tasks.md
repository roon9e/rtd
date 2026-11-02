# Tasks

## 1. Traits

- [ ] 1.1 Define `AuthKeyStore`/`LayerEvidenceStore`/`DeliveryStore` async traits and move the in-memory impls out of `LocalCore` (`rtd-server/src/store.rs`)

## 2. Backends

- [ ] 2.1 Add a PostgreSQL backend with in-repo migrations (`rtd-server/src/store/postgres.rs`)
- [ ] 2.2 Add Redis coordination (leases/ephemeral)

## 3. Wiring

- [ ] 3.1 Add storage selection flags/env to `rtd-edge` and default to in-memory (`rtd-edge/src/config.rs`, `service.rs`)

## 4. Verification

- [ ] 4.1 Verify auth keys and layer evidence survive a process restart with PostgreSQL (`rtd-server` tests)
- [ ] 4.2 Verify the in-memory default keeps the existing e2e green
- [ ] 4.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
