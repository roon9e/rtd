# Tasks

## 1. Snapshot

- [x] 1.1 Add `SessionData` (authorization key bytes, session id, salt) with a versioned magic-prefixed `encode`/`decode`; verify a round trip and that a truncated, foreign-magic or wrong-version blob fails closed (`session.rs`: `snapshot_round_trips_and_restores`, `malformed_snapshots_fail_closed`)
- [x] 1.2 Add `ClientSession::snapshot`/`from_data` (deriving the key fingerprint) and `Client::session_data`/`from_session`; verify a restored client reports the same session id, salt and key as the original (`session.rs` tests; `from_data` rebuilds the `AuthKey` with `Key::with_id()`)

## 2. Verification

- [x] 2.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`)
