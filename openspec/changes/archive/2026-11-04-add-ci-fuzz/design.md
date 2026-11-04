# Design

## Context

The decoders (`rtd-bin`, `rtd-proto`, `rtd-mt`, `rtd-tg`, `rtd-tlprofile`) all consume untrusted
bytes; the exchange consumes `resPQ`/`server_DH_params`. `td` fuzzes `handle_message`, the flow
and RSA.

## Goals / Non-Goals

**Goals**

- No panics or unbounded allocation on arbitrary input.
- Bounded-time CI coverage.

**Non-Goals**

- Exhaustive coverage; long-running campaigns.

## Decisions

### D1. One target per parsing surface

`decode_message`, `decode_container`, `decode_gzip`, `admit_tl`, `exchange_flow`, `rsa_pad`.
Each asserts only "no panic", with an allocation cap to catch decompression bombs.

### D2. Seed from real captures

Use `interop/vectors.json` and the e2e test traffic as seeds so the fuzzer explores valid
prefixes quickly.

### D3. Bounded CI run

Run each target for a fixed wall-clock budget in a non-blocking CI job, mirroring `td`.

## Risks / Trade-offs

- [Fuzzing needs nightly] → keep the job non-required and pinned to a nightly toolchain.
