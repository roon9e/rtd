# Tasks

## 1. Mux

- [ ] 1.1 Add a same-port peeker and a `PrefixedStream` preserving the buffered bytes (`rtd-server/src/ingress.rs`)
- [x] 1.2 Route WebSocket upgrades and explicit HTTP routes; default to the MTProto path

## 2. Edge wiring

- [ ] 2.1 Enable the mux in `rtd-edge` (health/metrics routes only; no server-info) (`rtd-edge/src/service.rs`)

## 3. Verification

- [ ] 3.1 Verify MTProto, WebSocket and HTTP each work on the same port (`rtd-server/tests/e2e.rs`)
- [ ] 3.2 Verify an obfuscated2 connection is not misclassified as HTTP
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
