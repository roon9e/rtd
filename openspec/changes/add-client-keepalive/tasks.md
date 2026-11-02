# Tasks

## 1. Ping loop

- [ ] 1.1 Send `ping`/`ping_delay_disconnect` on an interval and track `pong`; expose `ClientOptions` interval/timeout (`client.rs`)
- [ ] 1.2 Mark the connection dead when a `pong` is missed

## 2. Reconnect and retry

- [ ] 2.1 Reconnect with capped exponential backoff, reusing `SessionData` (`client.rs`, `async_client.rs`)
- [ ] 2.2 Resend requests that were never acknowledged; fail acknowledged-but-unanswered calls

## 3. Verification

- [ ] 3.1 Verify a missed pong triggers a reconnect and a queued call completes over the new connection (`tests/keepalive.rs`)
- [ ] 3.2 Verify backoff grows and is capped using `ManualClock`
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
