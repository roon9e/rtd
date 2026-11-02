# Tasks

## 1. Config

- [ ] 1.1 Add `--websocket-origin` (repeatable) / `RTD_EDGE_WEBSOCKET_ORIGINS` (`rtd-edge/src/config.rs`)
- [ ] 1.2 Add `websocket_origins` to `IngressConfig` (`rtd-server/src/ingress.rs`)

## 2. Enforcement

- [ ] 2.1 Enforce the allowlist at upgrade, allowing a missing `Origin` only when the list is empty

## 3. Verification

- [ ] 3.1 Verify an allowed origin upgrades and a disallowed one is rejected (`rtd-server` tests)
- [ ] 3.2 Verify the default (empty list) stays permissive
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
