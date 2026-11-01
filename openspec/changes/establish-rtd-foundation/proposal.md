# Proposal

## Why

`gramsrv` — a Telegram-compatible MTProto server — depends on `td`
(`github.com/iamxvbaba/td`, a fork of gotd/td) for its entire protocol core.
`td` is a client-oriented library: its multi-layer support reaches the server
only through a large generated Go sidecar, and it has **no server-side multi-DC
runtime** at all (`gramsrv` is single-DC and rewrites every media `dc_id` to its
own DC). We are building `rtd`: a from-scratch Rust implementation of `td`'s
server-facing core, designed from the start for **exact multi-layer operation**
and a **multi-DC server runtime**, so a Telegram-compatible server can be built
on it without inheriting those limits.

## What Changes

- **New Rust cargo workspace** (`rtd` facade plus domain crates) implementing the
  server-facing core of `td`: TL binary serialization, MTProto 2.0 message
  framing, cryptography, transports, the MTProto service (`mt`) layer, the
  canonical generated model, and a multi-layer dispatcher.
- **New Rust code-generation pipeline** — a standalone, xtask-style generator that
  consumes a versioned multi-layer manifest + policy and emits (1) the canonical
  value model and (2) a static per-`(profile, wire-id)` routing sidecar. Generated
  Rust is committed. Nothing about the number of profiles is hard-coded.
- **Multi-layer semantics preserved, Rust-idiomatic representation.** Exact-profile
  admission, stable semantic identity, per-profile encoding, frozen objects, and
  fail-closed handling of unknown layers are all retained — but expressed with
  Rust enums, generated `match` tables, zero-copy buffers and codec traits instead
  of a transliteration of Go's interface/mutable-buffer style.
- **New multi-DC server runtime**: DC identity/config/keys, DC-aware auth-key and
  session routing, `MIGRATE`/redirect errors, authorization transfer, and
  cross-DC delivery. These do not exist server-side in Go `gramsrv` today.
- **Sidecar edge service**: an rtd MTProto edge that a separate core (initially Go
  `gramsrv`) can drive over gRPC, carrying raw TL bytes plus admitted-call metadata
  (semantic method, negotiated layer, wire id). rtd owns the full edge — transport,
  key exchange, sessions, **multi-layer admission and layer binding** — so the core
  does not need the multi-layer machinery on the hot path.
- **Official-engine compatibility, as a hard gate**: rtd tracks
  `TelegramMessenger/mtproto-engine` — the official Rust MTProto 2.0 engine — as a
  first-class wire-compatibility target alongside Go `td`. Interoperability with that
  engine is an **acceptance requirement**, proven by an external-peer harness in
  milestone 1, not merely a goal.
- **Sans-IO protocol core**: the protocol crates expose byte-in/byte-out codecs and
  explicit state machines with **injected** time and randomness, owning no sockets or
  runtime; only the runtime crates own I/O. This matches the official engine's
  deterministic design and makes the core testable without a network.
- **Milestone 1 (this change)** delivers the workspace, the `bin`/`crypto`/`proto`
  primitives with test vectors, the auth-key exchange flow, the generator skeleton proven
  end-to-end on canonical layer 232, and the interop harness — including the external
  peer gate against the official engine's test server.
- **Out of scope**: the client library (`telegram.Client`, updates engine, auth
  flows, downloader/uploader) — the workspace is structured so it can be added
  later without rework, but it is not built now. Also out of scope: in-process
  Go↔Rust FFI/cgo, and rewriting Go `gramsrv`. `gramsrv` integration is supported
  only through the sidecar edge service described above. No source is copied from, and
  no dependency is taken on, the GPL-2.0 `TelegramMessenger/mtproto-engine`; it is a
  read-only compatibility reference.

## Capabilities

### New Capabilities
- `tl-codegen`: the Rust multi-layer code generator — manifest/policy/provenance
  inputs, canonical-model and profile-sidecar emission, plan deduplication, source
  budgets, and deterministic generation gates.
- `multi-layer-tl`: the runtime multi-layer model — exact-profile admission and
  dispatch, wire↔semantic identity, per-profile result/update encoding, frozen
  objects, and fail-closed behavior for unknown layers.
- `protocol-wire`: TL binary serialization and MTProto 2.0 framing — the `bin`
  buffer primitives, message ids/containers/gzip/rpc-result, and transport codecs.
- `protocol-crypto`: MTProto cryptography — AES-IGE encryption, keys/message-key
  KDF (v1 and v2), RSA exchange padding, DH/PQ checks, SRP, and auth-key derivation.
- `server-edge`: the MTProto server edge — connection lifecycle, transport
  detection, key exchange, auth-key/session registries, admission budgets, message
  id/seq validation, and service-message handling.
- `server-rpc`: server RPC dispatch — wrapper handling, layer binding, per-profile
  result encoding, and TL error mapping.
- `multi-dc`: DC identity, configuration and RSA keys, DC-aware routing and
  migration/redirects, authorization transfer, and cross-DC state/delivery.
- `edge-service`: the gRPC boundary that lets a separate core service (initially Go
  `gramsrv`) use rtd as its MTProto edge — TL bytes in and out plus admitted-call
  metadata for multi-layer dispatch and per-profile result encoding.
- `interop-testing`: wire-compatibility verification — cross-implementation interop
  tests against Go `td` and the official Rust engine, an external-peer harness, and
  shared cryptographic/protocol vectors. Official-engine interoperability is a hard
  acceptance gate.

### Modified Capabilities
<!-- None: rtd is a new project with no existing specs. -->

## Impact

- **New code**: the entire `rtd` Rust workspace under `C:\Users\roon9e\Documents\github\rtd`.
- **Inputs imported from `td`**: `_schema/layers/*.tl`, `_schema/layers/manifest.json`,
  `_schema/layers/policy.json` (with tdesktop provenance preserved).
- **Upstream references** (read-only): `C:\Users\roon9e\Documents\github\td` and
  `C:\Users\roon9e\Documents\github\gramsrv`.
- **External reference (read-only)**: `TelegramMessenger/mtproto-engine` (GPL-2.0) — used
  only as a compatibility target and behavioural reference, and its published binaries as
  an external test peer. No source is copied and no dependency is taken.
- **Toolchain**: Rust stable (1.99.x, edition 2024), tokio; Go (for the `td` vector helper);
  Node/OpenSpec for planning artifacts.
- **Licensing**: MIT OR Apache-2.0; `td`/`gotd` MIT notices retained for derived
  material.
- **Go `gramsrv` integration**: through the sidecar edge service only. `gramsrv` keeps
  its Go core and exchanges TL bytes plus admitted-call metadata with rtd over gRPC
  (matching the `gramsrv` v2 Edge/Core split). No in-process linking; the Go core may
  still use Go `td` for its own types and business logic.
