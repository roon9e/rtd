# Tasks

## 1. Delivery service

- [ ] 1.1 Add a `Delivery` trait + gRPC egress client with bearer token (`rtd-edge/src/delivery.rs`)
- [ ] 1.2 Add `egress.delivery` config (resolver/targets/token/timeout)

## 2. Executor

- [ ] 2.1 Add a delivery executor/codec that wraps results and updates for another DC
- [ ] 2.2 Add replay/retention with client-ack evidence and bounded retention (durable via storage)

## 3. Scheduling

- [ ] 3.1 Add a bulk scheduler and rpc-flight tracking for fair delivery

## 4. Verification

- [ ] 4.1 Verify a delivery to a disconnected target is replayed after reconnect (`rtd-edge` tests)
- [ ] 4.2 Verify retention is bounded and pruned after acknowledgement
- [ ] 4.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
