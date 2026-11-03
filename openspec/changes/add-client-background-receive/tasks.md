# Tasks

## 1. Split the transport

- [x] 1.1 Add `ReadTransport`/`WriteTransport` and `DuplexStream::split` (`transport.rs`)
- [ ] 1.2 Move `ClientSession` behind a lock or actor and expose frame read/write separately (`session.rs`, `client.rs`)

## 2. Reader and events

- [x] 2.1 Add a background `Reader` over a split transport (`transport.rs`)
- [ ] 2.2 Bridge events to `tokio::sync::mpsc` in `AsyncClient`, replacing per-call read ownership (`async_client.rs`)

## 3. Verification

- [ ] 3.1 Verify an update sent between calls is delivered without a pending call (`tests/updates.rs`)
- [ ] 3.2 Verify a dropped peer surfaces `Closed` once and the reader stops (`tests/round_trip.rs`)
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
