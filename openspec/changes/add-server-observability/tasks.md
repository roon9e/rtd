# Tasks

## 1. Metrics

- [ ] 1.1 Add a metrics registry with counters/histograms (`rtd-server/src/metrics.rs`)
- [ ] 1.2 Instrument connection, handshake, admission, dispatch and error paths

## 2. Exposition and identity

- [ ] 2.1 Add an HTTP exposition endpoint and a debug/observability addr to `rtd-edge` (`rtd-edge/src/config.rs`, `service.rs`)
- [ ] 2.2 Add instance identity + build info, logged at startup

## 3. Verification

- [ ] 3.1 Verify metrics increment on a session and are exposed over HTTP (`rtd-edge` tests)
- [ ] 3.2 Verify the identity appears in startup logs
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
