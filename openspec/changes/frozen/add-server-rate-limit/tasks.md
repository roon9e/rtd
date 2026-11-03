# Tasks

## 1. Limiter

- [x] 1.1 Add `RateLimiter` (token buckets keyed by method class + identity, clock-driven) (`rtd-server/src/ratelimit.rs`)
- [x] 1.2 Add sensitive-method classes and presets

## 2. Dispatch

- [x] 2.1 Apply the limiter before dispatch and return `FLOOD_WAIT_<n>` on refusal (`rpc.rs`)

## 3. Verification

- [x] 3.1 Verify a burst beyond the limit returns `FLOOD_WAIT` and recovers after the window (`rtd-server` tests)
- [x] 3.2 Verify service messages are not limited
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
