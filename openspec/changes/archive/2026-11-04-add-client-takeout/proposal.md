# Proposal

## Why

Data export requires wrapping every call in `account.initTakeoutSession`/`finishTakeoutSession`
via `invokeWithTakeout`. `rtd-client` does not implement it. `td` ships `telegram/takeout`
(`Run`, `Config`, `Client.InvokeWithTakeout`).

## What Changes

- `Takeout::run(config, f)`: initialize the takeout session, run the caller's closure with an
  invoker that wraps calls, then finish the session.
- A `Config` describing which data categories and file-size limits to export.

## Capabilities

### New Capabilities

- `client-takeout`: data-export session wrapper.

## Impact

- **Crates**: `rtd-client` (new `takeout` module).
- **Non-goals**: assembling the exported archive.
