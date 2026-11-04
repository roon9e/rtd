# Proposal

## Why

`rtd-crypto/src/srp.rs` implements only the **client** side of SRP (the `hash`/`new_hash`
proofs). Without the server side — computing the password verifier, deriving `srp_B`, and
verifying `M1` — `rtd` cannot serve `auth.checkPassword` and no end-to-end 2FA test is possible.
`td` has `crypto/srp.NewSRP` with both directions.

## What Changes

- Add a server-side SRP verifier to `rtd-crypto::srp`: derive the verifier from
  `PasswordKdfAlgo` (`password_hash`), derive `srp_B` from `g`,`p`, the verifier and the server
  random `b`, and verify the client proof `M1` (and produce `M2`).
- Add known-answer tests against Telegram's test vectors.

## Capabilities

### New Capabilities

- `srp-verifier`: server-side SRP-6a verification for Telegram 2FA.

## Impact

- **Crates**: `rtd-crypto` (`srp.rs`, `error.rs`).
- **Non-goals**: account state, password storage, or the `auth.checkPassword` RPC wiring
  (`add-client-2fa` and a server core handler).
