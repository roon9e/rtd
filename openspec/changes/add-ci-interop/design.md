# Design

## Context

The peer gate is env-gated and skips when `RTD_PEER_TESTSERVER` is unset, so CI never covers it.
The engine is GPL-2.0: it must be built from a separate checkout and executed as a peer only, not
linked or vendored.

## Goals / Non-Goals

**Goals**

- Prove rtd interoperates with the official engine and with the Go vectors on every push.
- Prove the edge drop-in end to end.

**Non-Goals**

- Shipping the engine binary or contacting the real Telegram servers.

## Decisions

### D1. Build the peer in a separate job step

Checkout `mtproto-engine` at a pinned ref, `cargo build --release -p mtproto-testserver`, export
`RTD_PEER_TESTSERVER`, then run `cargo test -p rtd-exchange --test peer_gate` and
`-p rtd-client --test peer_gate`. The engine is never added as a dependency.

### D2. Vectors job

Run `cd interop/vectors && go run .` and `git diff --exit-code ../vectors.json` to keep the
committed vectors fresh, then run the binary tests.

### D3. Drop-in job

Bring up the gramsrv v2 stack (or a fake core) and run an `rtd-client` -> `rtd-edge` smoke that
completes `help.getConfig`, gated on the pinned core image.

## Risks / Trade-offs

- [Heavy/flaky network] → pin versions and make the real-Telegram part optional/skipped.
