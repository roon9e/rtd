# Tasks

## 1. Token calls

- [ ] 1.1 Add `export_login_token`/`import_login_token`/`accept_login_token` (`client.rs`/`qr.rs`)
- [ ] 1.2 Handle `auth.LoginTokenMigrateTo` as a typed error until DC switching lands

## 2. Driver

- [ ] 2.1 Add a `QR` helper looping export -> token callback -> import until authorized

## 3. Verification

- [ ] 3.1 Verify a mock server drives export -> import -> authorization and emits the token callback (`tests/qr_login.rs`)
- [ ] 3.2 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
