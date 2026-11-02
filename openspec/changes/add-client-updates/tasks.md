# Tasks

## 1. Update dispatch

- [x] 1.1 Add `Client::set_update_handler` and dispatch non-result messages to it (`Updates`, or a bare `Update` wrapped in `Updates::updateShort`); verify a mock server that sends an `updateShort` before the `rpc_result` is observed by the handler (`client.rs`; `tests/updates.rs::an_in_band_update_is_delivered`)
- [x] 1.2 Verify an unrecognised non-result message is still ignored and does not disturb the awaited result (`tests/updates.rs::an_unknown_message_does_not_disturb_the_result`)

## 2. Verification

- [x] 2.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`)
