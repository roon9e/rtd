# Tasks

## 1. Config

- [x] 1.1 Add `--websocket-origin` (repeatable) / `RTD_EDGE_WEBSOCKET_ORIGINS` (`rtd-edge/src/config.rs`)
- [x] 1.2 Add `websocket_origins` to `IngressConfig` (`rtd-server/src/ingress.rs`)

## 2. Enforcement

- [x] 2.1 Enforce the allowlist at upgrade, allowing a missing `Origin` only when the list is empty

## 3. Verification

- [x] 3.1 Verify an allowed origin upgrades and a disallowed one is rejected (`rtd-server` tests)
- [x] 3.2 Verify the default (empty list) stays permissive
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
