# Tasks

## 1. Transport

- [ ] 1.1 Add a WebSocket byte-stream adapter implementing `Read + Write` (`transport.rs` or a `ws.rs` module)
- [ ] 1.2 Add a `ws://`/`wss://` dialer using the intermediate codec

## 2. Verification

- [ ] 2.1 Verify a WebSocket client completes the exchange and an RPC against the rtd edge with `--websocket` (`rtd-server/tests/e2e.rs` style)
- [ ] 2.2 Verify non-binary frames are rejected
- [ ] 2.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
