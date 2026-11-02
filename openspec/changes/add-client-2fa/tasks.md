# Tasks

## 1. Two-factor flow

- [ ] 1.1 Add `PasswordProvider` (constant/environment) and typed errors (`client.rs` or `auth.rs`)
- [ ] 1.2 Implement `check_password`: `account.getPassword` -> `srp::hash` -> `auth.checkPassword` (`client.rs`)

## 2. Recovery

- [ ] 2.1 Add `RequestPasswordRecovery`/`CheckRecoveryPassword`/`RecoverPassword` wrappers

## 3. Verification

- [ ] 3.1 Verify a mock server that answers `SESSION_PASSWORD_NEEDED` and verifies `M1` completes `checkPassword` (`tests/authorization.rs`)
- [ ] 3.2 Verify a wrong password returns a typed error
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
