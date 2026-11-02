# Tasks

## 1. Dialer abstraction

- [ ] 1.1 Add `Dialer`/`Obfuscator` in `rtd-transport` with `Plain`/`Secured`/`FakeTls` selection from `Secret` (`dialer.rs`)
- [ ] 1.2 Compose `faketls` over `obfuscated2` for `ee` secrets, mirroring `td`'s `obfuscator` package
- [ ] 1.3 Add multi-endpoint dial with fallback

## 2. Client integration

- [ ] 2.1 Replace `connect_tcp`'s single obfuscated2 path with the dialer; keep plain TCP when no secret (`rtd-client::transport.rs`)

## 3. Verification

- [ ] 3.1 Verify each secret mode selects the expected layer (`rtd-transport` tests)
- [ ] 3.2 Verify an `ee` client completes a FakeTLS handshake against the rtd edge (`rtd-server/tests/e2e.rs` style)
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
