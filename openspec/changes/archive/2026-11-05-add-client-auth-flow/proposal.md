# Proposal

## Why

`td`'s `telegram/auth` drives the whole login state machine (`Flow`, `Status`, sign-in, sign-up,
TOS, password reset/recovery) and `auth/srpguard` keeps the plaintext 2FA password in locked
memory. `rtd-client` only exposes the low-level `send_code`/`sign_in`/`check_password` calls.

## What Changes

- Add a `Flow` that reports status and drives send/resend code, sign-in, sign-up, TOS and 2FA
  through a `PasswordProvider`.
- Add a locked, zeroizing password buffer (`srpguard` equivalent) and accept it in
  `check_password`.

## Capabilities

### New Capabilities

- `client-auth-flow`: the high-level login state machine.
- `client-srpguard`: locked, zeroizing 2FA password handling.

## Impact

- **Crates**: `rtd-client`; optional `zeroize` dependency.
- **Non-goals**: QR login (already shipped separately).
