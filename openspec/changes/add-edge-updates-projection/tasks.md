# Tasks

## 1. Projection

- [x] 1.1 Add a pts projection with per-client/per-channel watermarks (`rtd-edge/src/projection.rs`)
- [ ] 1.2 Add per-channel delivery watermarks with resume on reconnect

## 2. Encoding

- [ ] 2.1 Encode projected updates at the session's negotiated layer via `rtd-tlprofile`

## 3. Verification

- [ ] 3.1 Verify updates are delivered in order and a reconnecting client resumes from its watermark (`rtd-edge` tests)
- [ ] 3.2 Verify a 229 and a 232 client both decode a projected update
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
