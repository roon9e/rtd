# Tasks

## 1. Generator

- [ ] 1.1 Add a production `ServerRng` generating a fresh valid `pq` and `p`/`g` (`rtd-exchange/src/rng.rs`)
- [ ] 1.2 Validate the prime/generator with `check_prime`/`check_gp`; precompute a small pool

## 2. Edge wiring

- [ ] 2.1 Select the production generator in `rtd-edge` by default; keep `TestServerRng` behind a flag (`rtd-edge/src/exchange.rs`)

## 3. Verification

- [ ] 3.1 Verify the production generator yields a client-acceptable `resPQ` (`rtd-exchange` tests)
- [ ] 3.2 Verify `rtd-edge` completes a live exchange with the production generator (`rtd-server/tests/e2e.rs` style)
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
