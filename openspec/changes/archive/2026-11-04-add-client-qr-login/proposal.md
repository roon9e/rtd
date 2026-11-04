# Proposal

## Why

QR login lets a user authenticate by scanning a code shown by a desktop client, using
`auth.exportLoginToken`/`importLoginToken`/`acceptLoginToken`. `rtd-client` does not implement
it. `td` ships `telegram/auth/qrlogin` (`QR.Export`/`Accept`/`Import`/`Auth`).

## What Changes

- `Client::export_login_token()` returning the token/URL and expiry.
- `Client::import_login_token(token)` and `accept_login_token(token)` for the receiving side.
- `QR` helper driving export -> poll/accept -> import with a `OnLoginToken` signal.

## Capabilities

### New Capabilities

- `client-qr-login`: QR-token login export, accept and import.

## Impact

- **Crates**: `rtd-client` (`client.rs` or a `qr.rs` module).
- **Non-goals**: rendering the QR image (callers do that).
