# Tasks

## 1. Session

- [ ] 1.1 Add `Takeout::run` initializing/finishing the session around a closure (`takeout.rs`)
- [ ] 1.2 Add the wrapping `TakeoutInvoker` (`invokeWithTakeout`) and a `Config`

## 2. Verification

- [ ] 2.1 Verify calls inside the closure are wrapped and the session is finished on success (`takeout` tests)
- [ ] 2.2 Verify the session is finished on error
- [ ] 2.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
