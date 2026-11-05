# Tasks

## 1. Flow

- [ ] 1.1 Add `Flow` and `Status` over the existing auth calls
- [ ] 1.2 Add sign-up (`auth.signUp`) and TOS (`help.acceptTermsOfService`) steps
- [ ] 1.3 Add password reset/recovery (`auth.requestPasswordRecovery`/`checkRecoveryPassword`/`recoverPassword`)

## 2. srpguard

- [ ] 2.1 Add a locked, zeroizing password buffer
- [ ] 2.2 Accept it in `check_password`

## 3. Verification

- [ ] 3.1 Verify the flow against a mock sign-in server
- [ ] 3.2 Verify the buffer is cleared on drop
