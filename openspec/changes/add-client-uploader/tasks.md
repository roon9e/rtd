# Tasks

## 1. Core

- [x] 1.1 Add `Uploader` with small/big selection and `part_size` growth (`uploader/mod.rs`)
- [ ] 1.2 Add `Source` (bytes, reader, HTTP) and `Progress`

## 2. Concurrency

- [ ] 2.1 Upload big-file parts concurrently through the pool with a bounded worker count

## 3. Verification

- [x] 3.1 Verify a large file stays within 3999 parts and the part size grows (`uploader` tests)
- [ ] 3.2 Verify progress is reported monotonically and cancellation stops the upload
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
