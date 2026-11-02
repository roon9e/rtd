# Tasks

## 1. Verifier

- [ ] 1.1 Add `Verifier` and `verifier(password, Input)` deriving `v = g^x mod p` for `PasswordKdfAlgoSHA256...` (`srp.rs`)
- [ ] 1.2 Add server side: given `v`, random `b`, and client `A`, derive `B` and a challenge

## 2. Proof verification

- [ ] 2.1 Verify `M1` in constant time and produce `M2` (`srp.rs`)
- [ ] 2.2 Validate client `A` with `check_dh` and reject `B ≡ 0 (mod p)`

## 3. Verification

- [ ] 3.1 Verify known-answer vectors (client `hash` against the new `verifier`) (`srp.rs` tests)
- [ ] 3.2 Verify a wrong password fails `M1` verification
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
