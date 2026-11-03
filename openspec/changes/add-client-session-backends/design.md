# Design

## Context

`SessionData` already round-trips (auth key, session id, salts). `td`'s `session.Data` adds DC
address, config and a JSON loader; `session/tdesktop` decrypts `key_datas` with `local_key`.

## Goals / Non-Goals

**Goals**

- A stable `Storage` trait plus file and memory backends.
- Read compatibility with Telethon and Telegram Desktop sessions.

**Non-Goals**

- WASM local storage.

## Decisions

### D1. Trait first, backends second

`Storage { fn load(&self) -> Result<Option<SessionData>>; fn store(&self, d: &SessionData); fn delete(&self); }`.
`FileStorage` writes atomically (temp + rename, mode 0600); `MemoryStorage` clones.

### D2. Importers are read-only adapters

`TelethonSession::decode(string)` and `TDesktopSession::load(dir, password)` produce a
`SessionData`; they never write the foreign format.

### D3. Keep the versioned blob

The on-disk format stays the versioned `RTDS` blob so existing sessions keep working.

## Risks / Trade-offs

- [TDesktop key derivation corner cases] → cover with the `td` fixtures and a modern-layout
  preference (as `td` did in #825).
