# Tasks

## 1. Storage

- [x] 1.1 Add the `Storage` trait and `FileStorage` (atomic, mode 0600) and `MemoryStorage` (`session/`)
- [x] 1.2 Keep the versioned `RTDS` blob as the on-disk format

## 2. Importers

- [x] 2.1 Add a Telethon string-session importer
- [x] 2.2 Add a Telegram Desktop importer (`tdata`, decrypt with the derived local key)

## 3. Verification

- [x] 3.1 Verify file round-trip and atomic write (`tests/session.rs`)
- [ ] 3.2 Verify Telethon and TDesktop fixtures decode to a usable `SessionData`
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
