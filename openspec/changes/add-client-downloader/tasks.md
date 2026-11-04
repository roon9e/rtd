# Tasks

## 1. Core

- [x] 1.1 Add a sequence `Downloader` with aligned part sizing (`downloader/mod.rs`)
- [x] 1.2 Implement the master ranged read (`upload.getFile`)

## 2. CDN

- [ ] 2.1 Implement the CDN redirect state machine with token refresh and a verified-window cache (`downloader/cdn.rs`)

## 3. Verification

- [ ] 3.1 Verify a file downloads intact with verification enabled (`downloader` tests)
- [ ] 3.2 Verify a CDN redirect is followed and hashes are checked
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
