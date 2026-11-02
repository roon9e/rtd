# Design

## Context

`rtd_crypto::srp::{Input, Answer, hash}` already computes the client proof. `account.getPassword`
returns `account.Password` carrying `current_algo` (`PasswordKdfAlgoSHA256...`), `srp_B` and
`srp_id`. The client assembles `Input { salt1, salt2, g, p }` from `current_algo`, calls `hash`,
and sends `auth.checkPassword` with `InputCheckPasswordSRP`.

## Goals / Non-Goals

**Goals**

- A typed 2FA flow reachable from the sync and async clients.
- Deterministic tests against a mock server that verifies `M1` with the new server verifier.

**Non-Goals**

- Sign-up, QR login, password policy, secure memory.

## Decisions

### D1. Password provider, not a captured string

`PasswordProvider::password(&self) -> Option<String>` (constant or environment). The flow calls
it only when `SESSION_PASSWORD_NEEDED` is seen, mirroring `td`'s `PasswordHashProvider`.

### D2. Reuse the client SRP primitive unchanged

`check_password` builds `Input` from `current_algo` and calls `rtd_crypto::srp::hash`; no new
crypto in the client.

### D3. Recovery helpers are thin wrappers

`RequestPasswordRecovery`/`CheckRecoveryPassword`/`RecoverPassword` wrap the corresponding
`auth.*` methods with typed results, matching the `td` helpers added in #615.

## Risks / Trade-offs

- [Passwords in memory] → document that `srpguard`-equivalent memory locking is out of scope
  here and tracked separately.
