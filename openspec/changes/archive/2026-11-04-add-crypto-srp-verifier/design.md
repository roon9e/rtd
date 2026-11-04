# Design

## Context

`rtd-crypto::srp` already has the client primitives: `hash(password, Input) -> Answer`
(`A`, `M1`), with `SH = H(salt ++ data ++ salt)` and PBKDF2. The server needs the inverse:
turn a stored password into a verifier and check the client's `M1`. `td`'s `crypto/srp`
implements both in one type.

## Goals / Non-Goals

**Goals**

- Deterministic verifier derivation and `M1` verification using `num_bigint`.
- Interoperability with the client half already shipped.

**Non-Goals**

- Password policy, storage, or rate limiting.

## Decisions

### D1. Verifier from `PasswordKdfAlgo`

For `PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow`, compute
`x = PBKDF2(H(salt1 ++ password ++ salt1), salt2, 100000)`, then `v = g^x mod p`. This is the
same pipeline `new_hash` uses, so a shared helper avoids divergence.

### D2. `srp_B` and `M1` verification

Given random `b`, `B = (k*v + g^b) mod p` where `k = H(p ++ g)`; reject `B ≡ 0 (mod p)`. On
`M1`, compute `S = (A * v^u)^b mod p` with `u = H(A ++ B)` and compare the derived `M1` in
constant time. Produce `M2` for the client.

### D3. Reuse `check_dh` on the server

Validate client `A` with the existing `check_dh` bounds before doing modular exponentiation.

## Risks / Trade-offs

- [Constant-time comparison] → use the same constant-time equality as the existing crypto code;
  never `==` on bytes for proofs.
