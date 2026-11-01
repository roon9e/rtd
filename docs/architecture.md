# Architecture

This is a short orientation. The authoritative plan, decisions and requirements live in
[`openspec/changes/establish-rtd-foundation/`](../openspec/changes/establish-rtd-foundation/)
(`proposal.md`, `design.md`, `specs/`).

## Layering

```
                 +--------------------------------------------------+
   runtime       |  rtd-server  /  rtd-edge                         |
                 |  tokio, sockets, timers, gRPC boundary           |
                 +----------------------+---------------------------+
                                        |  sans-IO
                 +----------------------v---------------------------+
   multi-layer   |  rtd-tlprofile  —  admission, layer binding,     |
                 |  per-profile encode/decode, frozen objects       |
                 +----------------------+---------------------------+
                 +----------------------v---------------------------+
   model         |  rtd-tg (canonical)   rtd-mt (service schema)    |
                 +----------------------+---------------------------+
   protocol      |  rtd-bin · rtd-crypto · rtd-proto · rtd-transport|
                 |  rtd-error · rtd-util (injected clock/randomness)|
                 +--------------------------------------------------+
```

## Sans-IO core

The protocol crates own no sockets and no runtime and never read ambient time or
randomness. Time enters through `rtd_util::Clock` and randomness through `rtd_util::Random`;
`SystemClock`/`SystemRandom` are the production sources and `ManualClock`/
`DeterministicRandom` the test sources. This makes the protocol deterministic, fuzzable
and testable without a network, and lets the same core back a client, a server, or a test
harness. Only `rtd-server`/`rtd-edge` own I/O (tokio), timers and the gRPC boundary.

## Multi-layer model

One **canonical** value model (`rtd-tg`) plus a generated sidecar (`rtd-tlprofile`) that
knows every supported layer. For each `(profile, wire id)` pair the generator precomputes a
static route with one of six modes — `direct`, `retag`, `rewrite`, `policy`,
`profile-only`, `unavailable` — and emits it as a `match` table. There is no runtime schema
and no byte-to-byte transcoder; unsupported layers **fail closed** rather than being served
with the canonical layer. Results and updates are encoded per the session's negotiated
layer, and a value that a layer cannot represent is an error, never a canonical fallback.

Layers are added by extending the manifest; nothing about the number of profiles is
hard-coded. The pipeline is staged: prove the machinery on the canonical layer (232), then
add 231 → 225.

## Generation workflow

`rtd-codegen` is a standalone tool (not a `build.rs`). It reads `schema/` — the imported,
provenance-locked layer schemas, `manifest.json` and `policy.json` — plus `schema/rtd.json`
(the enabled layers), and emits the canonical model, the profile sidecar and the MTProto
service schema. Output is deterministic and atomic, plans are deduplicated by digest, a
source-size budget is enforced, and the result is **committed**. CI re-runs the generator
and fails if the committed output differs.

## Edge service boundary

A separate core service (initially Go `gramsrv`) can use rtd as its MTProto edge over a
versioned gRPC contract: rtd owns the full edge — transport, key exchange, sessions,
multi-layer admission and layer binding — and exchanges raw TL bytes plus admitted-call
metadata with the core, which returns canonical results that the edge encodes per the
session's layer. This mirrors `gramsrv` v2's Edge/Core split.

## Multi-DC

The server runtime keeps one authoritative shared store (PostgreSQL + Redis) with
datacenters as edge replicas; a fixed primary DC handles login; delivery goes through a
`Delivery` abstraction with a durable outbox. The full design is the "Multi-DC architecture"
section of the change's `design.md`, with decisions D5, D9, D10, D15 and D16.

## Compatibility and testing

Wire compatibility is verified by shared vectors emitted from Go `td`
(`interop/vectors`) and by an **external-peer harness** against the official Rust engine
(`TelegramMessenger/mtproto-engine`), which is a hard acceptance gate. The official engine
is read-only: no source is copied and no dependency is taken.
