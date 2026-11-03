# Tasks

## 1. Service

- [x] 1.1 Add a `FileService` trait and a `MemoryFileService` (`rtd-edge/src/file.rs`)
- [ ] 1.2 Add `file_data` config (resolver/targets/token/timeout) (`rtd-edge/src/config.rs`)

## 2. Routing

- [ ] 2.1 Intercept file methods after admission and dispatch to `FileService`, wrapping the result
- [ ] 2.2 Fall back to the core on a file-service error

## 3. Verification

- [ ] 3.1 Verify `upload.getFile` is served without a core call against a fake file service (`rtd-edge` tests)
- [ ] 3.2 Verify a file-service failure falls back to the core
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
