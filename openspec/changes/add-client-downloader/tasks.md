# Tasks

## 1. Core

- [ ] 1.1 Add `Downloader`/`Builder` with part sizing, threads, verify and sinks (`downloader/mod.rs`)
- [ ] 1.2 Implement the master (precise range) and web schemas

## 2. CDN

- [ ] 2.1 Implement the CDN redirect state machine with token refresh and a verified-window cache (`downloader/cdn.rs`)

## 3. Verification

- [ ] 3.1 Verify a file downloads intact with verification enabled (`downloader` tests)
- [ ] 3.2 Verify a CDN redirect is followed and hashes are checked
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
