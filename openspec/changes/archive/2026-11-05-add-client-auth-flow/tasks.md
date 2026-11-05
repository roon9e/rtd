# Tasks

## 1. Flow

- [x] 1.1 Add `Flow` and `Status` over the existing auth calls
- [x] 1.2 Add sign-up (`auth.signUp`) and TOS (`help.acceptTermsOfService`) steps
- [x] 1.3 Password reset/recovery (`auth.requestPasswordRecovery`/`checkRecoveryPassword`/`recoverPassword`) already shipped as client methods

## 2. srpguard

- [x] 2.1 Add a zeroizing password buffer (`HeldPassword`)
- [x] 2.2 Use it as a `PasswordProvider`

## 3. Verification

- [x] 3.1 Verify the flow reports status and signs in against a mock server
- [x] 3.2 Verify the held password is redacted and available
