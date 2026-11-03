# Tasks

## 1. Sequence engines

- [x] 1.1 Add a generic `SequenceBox` with gap buffer and gap timeout (`updates/mod.rs`)
- [x] 1.2 Add `Manager` implementing the update handler and wiring the boxes (`updates/mod.rs`)

## 2. Recovery

- [ ] 2.1 Implement `updates.getDifference` handling (Empty/Slice/TooLong) (`updates/difference.rs`)
- [ ] 2.2 Implement per-channel `updates.getChannelDifference` with a concurrency cap and access-hash stores

## 3. State and callbacks

- [x] 3.1 Add `StateStorage` (in-memory default) and the too-long/load-failed callbacks

## 4. Verification

- [ ] 4.1 Verify a gapped sequence triggers a difference and converges (`tests/updates.rs`)
- [ ] 4.2 Verify a channel difference is requested once per gap and updates apply in order
- [ ] 4.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
