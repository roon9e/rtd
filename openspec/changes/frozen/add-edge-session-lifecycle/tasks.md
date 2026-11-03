# Tasks

## 1. Lifecycle

- [x] 1.1 Add `SessionState` (provisional/ready/revoked), `state`/`set_state`/`revoke` (`rtd-server/src/session.rs`)
- [ ] 1.2 Add provisional revocation with a confirmation deadline

## 2. Shutdown and ack

- [ ] 2.1 Add a shutdown gate with a drain barrier (bounded) around `Server::serve` (`rtd-server/src/actor.rs`, `rtd-edge/src/service.rs`)
- [ ] 2.2 Add a quick-ack deadline and pending-outbound flush

## 3. Verification

- [ ] 3.1 Verify a provisional session is revoked when never confirmed (`rtd-server` tests)
- [ ] 3.2 Verify shutdown drains in-flight calls and bounds the wait
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
