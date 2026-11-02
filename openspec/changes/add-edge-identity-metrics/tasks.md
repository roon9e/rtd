# Tasks

## 1. Identity

- [ ] 1.1 Add instance identity (config/env or random) and compiled-in build info (`rtd-edge/src/config.rs`, `main.rs`)
- [ ] 1.2 Attach the instance id as gRPC metadata on core-exec/egress calls

## 2. Runtime metrics

- [ ] 2.1 Register connection/session/in-flight/byte metrics in the shared registry
- [ ] 2.2 Add a debug addr for profiling

## 3. Verification

- [ ] 3.1 Verify identity and build info appear in the banner and on `/buildinfo` (`rtd-edge` tests)
- [ ] 3.2 Verify runtime metrics increment with traffic
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
