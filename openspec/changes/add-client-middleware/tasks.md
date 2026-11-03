# Tasks

## 1. Chain

- [x] 1.1 Add the `Invoker` trait and middleware chaining; make `Client` implement `Invoker` (`middleware.rs`)
- [x] 1.2 Add `ClientOptions.middlewares`

## 2. Decorators

- [x] 2.1 Add `FloodWait` retry using `Clock` and a bounded attempt count
- [x] 2.2 Add a simple rate-limit middleware and a no-op tracing hook

## 3. Verification

- [x] 3.1 Verify a `FLOOD_WAIT_3` is retried after 3 s of manual-clock time and then succeeds (`tests/middleware.rs`)
- [x] 3.2 Verify retries are bounded and the final error is surfaced
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
