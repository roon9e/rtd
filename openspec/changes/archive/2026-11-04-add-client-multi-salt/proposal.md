# Proposal

## Why

`ClientSession` holds a single server salt. Real Telegram servers rotate salts and answer
`get_future_salts` with a set carrying `valid_until`; `td` keeps a salt store and selects by
clock (`mtproto/salt.go`, `handle_future_salts.go`). Without proactive refresh, a long-lived
client will have every message rejected with `bad_server_salt`.

## What Changes

- Replace the single salt with a salt store `Vec<(valid_until, salt)>` selected by the injected
  clock, mirroring `td`'s `salts.Get`/`Store`/`Reset`.
- Send `get_future_salts` (present in `rtd-mt`) and handle the `future_salts` service reply
  (id `0xae500895`, not an `rpc_result`).
- Adopt `bad_server_salt` responses and fall back to the newest valid salt.

## Capabilities

### New Capabilities

- `client-multi-salt`: a clock-driven server-salt store with proactive `get_future_salts`.

## Impact

- **Crates**: `rtd-client` (`session.rs`, `client.rs`); uses `rtd-mt::Get_future_salts` and the
  existing `future_salts` object.
- **Non-goals**: server-side salt issuance (already answered by `rtd-server`).
