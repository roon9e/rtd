# Proposal

## Why

After `sendCode`/`signIn`, an account with a password answers `SESSION_PASSWORD_NEEDED`; the
client must run `account.getPassword` -> SRP -> `auth.checkPassword`. `rtd-client` has no 2FA
path, so such accounts cannot sign in. `td` implements this in `telegram/auth/password.go`
using `crypto/srp`.

## What Changes

- Typed `Client::check_password`: `account.getPassword` -> `account.Password` ->
  `rtd_crypto::srp::hash` -> `InputCheckPasswordSRP { srp_id, a, m1 }` -> `auth.checkPassword`.
- A password provider abstraction (constant/environment) and typed errors
  (`ErrPasswordAuthNeeded`/`ErrPasswordInvalid`).
- Password recovery helpers (`RequestPasswordRecovery`/`CheckRecoveryPassword`/`RecoverPassword`).

## Capabilities

### New Capabilities

- `client-2fa`: two-factor (SRP) sign-in and password recovery.

## Impact

- **Crates**: `rtd-client`; depends on `add-crypto-srp-verifier` for tests and on the existing
  client SRP hashing.
- **Non-goals**: secure-memory handling (`srpguard`) and server-side password storage.
