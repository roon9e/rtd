# Tasks

## 1. Salt store

- [x] 1.1 Add `SaltStore` (`get`/`insert`/`reset`) selected by the injected clock; replace the single salt in `ClientSession` (`session.rs`)
- [x] 1.2 Send `get_future_salts` when the newest salt nears expiry, guarded against concurrent sends (`client.rs`)

## 2. Service reply

- [x] 2.1 Classify and handle the `future_salts` service reply (id `0xae500895`) before `rpc_result` (`client.rs`)
- [x] 2.2 Adopt `bad_server_salt` into the store and resend the affected message

## 3. Verification

- [x] 3.1 Verify selection picks the newest unexpired salt and ignores expired entries (`tests/`)
- [x] 3.2 Verify a `future_salts` reply is stored and used by the next message
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
