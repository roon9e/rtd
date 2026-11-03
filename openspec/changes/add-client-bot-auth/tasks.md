# Tasks

## 1. Bot sign-in

- [x] 1.1 Add `Client::bot_sign_in` invoking `auth.importBotAuthorization` and returning `auth.Authorization` (`client.rs`)
- [x] 1.2 Add a bot-token option (and environment fallback), never logged

## 2. Verification

- [ ] 2.1 Verify a mock server drives `initialize` -> `bot_sign_in` and returns the typed authorization (`tests/authorization.rs`)
- [ ] 2.2 Verify an invalid token surfaces a typed error
- [ ] 2.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
