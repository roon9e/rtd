# Tasks

## 1. Async façade

- [x] 1.1 Add `AsyncClient` with `spawn` (owning a blocking actor thread over the synchronous `Client`) and `async` `initialize`/`invoke`, with startup errors surfaced synchronously and shutdown on drop; verify an async call completes (`async_client.rs`; tests in `round_trip.rs`: `async_client_completes_exchange_and_invokes`, `async_client_reports_a_startup_failure`)
- [x] 1.2 Add `AsyncClient::connect_tcp` (dial + spawn); verify the async round trip against an in-process mock server over a duplex driven on a current-thread runtime (`AsyncClient::spawn` over `DuplexStream`; tests use `block_on` to avoid a proc-macro dependency)

## 2. Verification

- [x] 2.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`)
