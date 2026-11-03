# Tasks

## 1. Verifier

- [x] 1.1 Add `Verifier` and `verifier(password, Input)` deriving `v = g^x mod p` for `PasswordKdfAlgoSHA256...` (`srp.rs`)
- [x] 1.2 Add server side: given `v`, random `b`, and client `A`, derive `B` and a challenge

## 2. Proof verification

- [x] 2.1 Verify `M1` in constant time and produce `M2` (`srp.rs`)
- [x] 2.2 Validate client `A` with `check_dh` and reject `B ≡ 0 (mod p)`

## 3. Verification

- [x] 3.1 Verify known-answer vectors (client `hash` against the new `verifier`) (`srp.rs` tests)
- [x] 3.2 Verify a wrong password fails `M1` verification
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
