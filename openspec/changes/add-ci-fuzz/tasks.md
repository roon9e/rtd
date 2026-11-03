# Tasks

## 1. Targets

- [x] 1.1 Add `cargo-fuzz` targets for message/container/gzip and TL admission (`fuzz/fuzz_targets/`)
- [ ] 1.2 Cap allocations in the decompression paths so bombs are caught

## 2. Seeds

- [ ] 2.1 Seed corpora from `interop/vectors.json` and e2e captures

## 3. CI

- [ ] 3.1 Add a bounded-time fuzz CI job (nightly toolchain, non-required)

## 4. Verification

- [ ] 4.1 Verify each target runs cleanly for a short local campaign
- [ ] 4.2 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
