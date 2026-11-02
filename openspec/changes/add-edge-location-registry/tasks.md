# Tasks

## 1. Provider

- [ ] 1.1 Add a `LocationProvider` chain with TTL cache and circuit breaker (`rtd-edge/src/location.rs`)
- [ ] 1.2 Add a local/remote provider implementation

## 2. Registry

- [ ] 2.1 Add an edge location registry with TTL + heartbeat keyed by instance id
- [ ] 2.2 Feed location into `help.getConfig` DC hints

## 3. Verification

- [ ] 3.1 Verify a client IP resolves to a location via cache and that failures degrade gracefully (`location` tests)
- [ ] 3.2 Verify a stale location entry expires
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
