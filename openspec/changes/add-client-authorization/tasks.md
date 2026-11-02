# Tasks

## 1. Authorization calls

- [x] 1.1 Add `Client::send_code` and `Client::sign_in` typed over `auth.sendCode`/`auth.signIn`; verify a mock server drives `initialize` -> `sendCode` -> `signIn` and the typed results are returned, and that an unexpected result constructor fails closed (`client.rs`; `tests/authorization.rs`: `login_flow_returns_typed_results`, `an_unexpected_result_fails_closed`)
- [x] 1.2 Verify the same methods are reachable through the async façade's generic `invoke` (`tests/authorization.rs::authorization_is_reachable_through_the_async_facade`)

## 2. Verification

- [x] 2.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`)
