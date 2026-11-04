# Tasks

## 1. Two-factor flow

- [x] 1.1 Add `PasswordProvider` (constant/environment) and typed errors (`client.rs` or `auth.rs`)
- [x] 1.2 Implement `check_password`: `account.getPassword` -> `srp::hash` -> `auth.checkPassword` (`client.rs`)

## 2. Recovery

- [x] 2.1 Add `request_password_recovery`/`check_recovery_password`/`recover_password` wrappers

## 3. Verification

- [x] 3.1 Verify a mock server that answers `SESSION_PASSWORD_NEEDED` and verifies `M1` completes `checkPassword` (`tests/authorization.rs`)
- [x] 3.2 Verify a wrong password returns a typed error
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
