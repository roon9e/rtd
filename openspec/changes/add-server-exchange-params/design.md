# Design

## Context

`rtd-exchange` drives both client and server flows; the server needs `pq`, `p`, `g` and the RSA
key. `TestServerRng` fixes these for deterministic tests. `rtd-crypto` already has `decompose_pq`
and `check_prime`/`check_gp`.

## Goals / Non-Goals

**Goals**

- Fresh, valid parameters per handshake, verifiable by the client.
- Determinism under `DeterministicRandom` for tests.

**Non-Goals**

- Changing the exchange protocol or the client flow.

## Decisions

### D1. Random 63-bit `pq` from two primes

Generate `p`,`q` in the client-accepted range (`p < q`), compute `pq = p*q`, and reject values
outside the protocol bounds, mirroring `td`'s `pq`.

### D2. Search for a safe prime `p` with generator `g`

Search `p = 2q + 1` (or the protocol's `known prime` optimization) with `g` in `{2,3,4,5,6,7}`,
validated by `check_prime`/`check_gp`. Cache a small pool to bound handshake latency.

### D3. A switch, not a replacement

`ServerExchange` takes a `Box<dyn ServerRng>`; `rtd-edge` uses the production generator by
default and exposes `--exchange-test-params` for tests/e2e.

## Risks / Trade-offs

- [Prime search cost] → precompute a small pool at startup and reuse across handshakes.
