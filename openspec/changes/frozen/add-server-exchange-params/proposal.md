# Proposal

## Why

The server side of the auth-key exchange uses `rtd-exchange`'s fixed `TestServerRng` for its PQ
and prime. A fixed `pq` means the same `resPQ` every handshake — trivially fingerprintable and
unsafe for anything but tests. `td` generates fresh PQ and a valid 2048-bit safe prime per
handshake.

## What Changes

- A production `ServerRng` that generates a fresh `pq` (two random primes) and a valid
  `p`/`g` per exchange, using the injected randomness.
- Prime validation reuse (`check_prime`/`check_gp`) before serving, with a bounded search.
- `rtd-edge` selects the production generator when not explicitly told to use the test one.

## Capabilities

### New Capabilities

- `server-exchange-params`: production Diffie-Hellman parameters for the server exchange.

## Impact

- **Crates**: `rtd-exchange` (`rng.rs`), `rtd-edge` (`exchange.rs`).
- **Non-goals**: rotating RSA keys (already loaded from PEM).
