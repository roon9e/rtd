# rtd

A from-scratch Rust implementation of the [MTProto 2.0](https://core.telegram.org/mtproto)
protocol and the Telegram API **client library**, targeting feature parity with
[`td`](https://github.com/iamxvbaba/td) (a fork of [`gotd/td`](https://github.com/gotd/td)).

`rtd` is a clean-room, wire-compatible reimplementation. It is **a library**: the
sans-IO protocol core plus a client runtime. It was validated end to end as an
MTProto **edge** inside a gramsrv v2 stack (the added server/edge crates are
frozen — see [Direction](#direction)).

> **Status: `td` client-library parity complete.** The sans-IO protocol core
> (serialization, crypto, framing, transports, MTProxy obfuscation, injectable
> clock/randomness, the Go cross-check harness, and the generated
> `rtd-tg`/`rtd-mt` value and service models) and the client library
> (`rtd-client`) are implemented. Coverage is estimated at **~93–97%** by
> weighted surface (≈100% of the packages that map to a client-library concern);
> the only accepted divergence is full `tdp` field fidelity (byte/date rendering).
> See [`docs/td-parity.md`](docs/td-parity.md) for the mapping and estimate.

## Direction

- **Library first.** The goal is full `td`-parity: a Rust client library and the
  protocol core underneath it.
- **No server storage layer.** PostgreSQL/Redis-backed storage is out of scope.
- **Server/edge frozen.** `rtd-server`, `rtd-edge` and `rtd-edge-proto` remain in
  the tree, frozen, so the compatibility edge image stays buildable; their
  proposals live in [`openspec/changes/frozen/`](openspec/changes/frozen/) until
  a future, separate full Rust server replaces them. Their capability specs stay
  deployed under [`openspec/specs/`](openspec/specs/).

## Crate layout

Protocol core (sans-IO, no runtime, no I/O):

| Crate | Role |
|---|---|
| `rtd-bin` | TL binary serialization primitives |
| `rtd-crypto` | MTProto cryptography (AES-IGE, KDF v1/v2, RSA_PAD, DH/PQ, SRP client + server verifier) |
| `rtd-proto` | Message framing, codecs, service primitives |
| `rtd-transport` | obfuscated2 and FakeTLS framing, MTProxy dialer |
| `rtd-mt` | generated MTProto service schema |
| `rtd-tg` | generated canonical Telegram value model |
| `rtd-tlprofile` | generated multi-layer routing + admission runtime |
| `rtd-error` | Telegram TL error parsing |
| `rtd-util` | injectable clock and randomness |
| `rtd-exchange` | authorization-key exchange (client and server flows) |
| `rtd-tmap` | wire-id to name registry |

Client library:

| Crate | Role |
|---|---|
| `rtd-client` | MTProto 2.0 client runtime: sync core + tokio async façade, plus auth (Flow/2FA/bot/QR), MTProxy, WebSocket, middleware, pooling, DC resolution, session backends, uploader/downloader, message/media/rich builders, markup, entity builder, pagination/search, peers, takeout, media helpers, file-id/thumbnail, JSON/`tdp`, constants, updates, calls/media, and env helpers |

Tooling:

| Crate | Role |
|---|---|
| `rtd-codegen` | the multi-layer TL code generator (tool) |
| `rtd-layerhooks`, `rtd-layerhooks-registry` | hand-written per-layer conversion hooks |
| `rtd` | facade re-exporting the public surface |
| `fuzz` | `cargo-fuzz` targets (standalone, not a workspace member) |

Frozen (out of scope, kept for the compatibility image):

| Crate | Role |
|---|---|
| `rtd-server` | server edge, RPC dispatch (frozen) |
| `rtd-edge` | the sidecar edge service binary (frozen) |
| `rtd-edge-proto` | gRPC Edge↔Core contract (frozen) |

## Build and test

```console
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

### Optional features

`rtd-client` enables the WebRTC media backend by default, mirroring `td`'s direct `pion/webrtc`
dependency; disable it with `default-features = false`. Two further integrations are opt-in:

| Feature | Crate | Effect |
|---|---|---|
| `media-webrtc` (default) | `rtd-client` | `WebRtcMedia` audio backend for calls |
| `otel` | `rtd-client` | `OtelTracer`/`OtelMetrics` over the global OpenTelemetry provider |
| `ntp` | `rtd-client` (from `rtd-util`) | an NTP-calibrated `NtpClock` |

Regenerate the committed generated sources (must produce no diff):

```console
cargo run -p rtd-codegen -- generate
git diff --exit-code
```

### Cross-check vectors and the external-peer gate

`interop/vectors` is a small Go program that emits protocol vectors from the Go
reference; the Rust tests consume the committed `interop/vectors.json`. The
official Rust engine (`TelegramMessenger/mtproto-engine`) is used as an external
test peer in the `peer_gate` tests (never linked or vendored — it is GPL-2.0).
Both run on demand via the manual **Interop** workflow
([`.github/workflows/interop.yml`](.github/workflows/interop.yml)):

```console
gh workflow run Interop
```

Without `RTD_PEER_TESTSERVER` the peer gate is skipped locally.

## Workspace conventions

- Edition 2024; the minimum supported Rust version tracks the latest stable toolchain.
- `unsafe` code is forbidden workspace-wide.
- Public items are documented; clippy warnings are denied.
- Generated crates are produced by `rtd-codegen` and never edited by hand.

## Compatibility targets

- [`td`](https://github.com/iamxvbaba/td) — the Go protocol library this reimplements.
- [`TelegramMessenger/mtproto-engine`](https://github.com/TelegramMessenger/mtproto-engine)
  — the official Rust MTProto engine; a **read-only** compatibility target and the
  external peer for interop tests. It is GPL-2.0, so no source is copied and no
  dependency is taken.

## Planning

Plans, specs and decisions live under [`openspec/`](openspec/). See
[`openspec/config.yaml`](openspec/config.yaml) for the project's frozen decisions, the deployed
capability specs under [`openspec/specs/`](openspec/specs/), and
[`docs/td-parity.md`](docs/td-parity.md) for the `td` mapping and coverage estimate. The active
`td`-parity backlog is complete; only [`openspec/changes/frozen/`](openspec/changes/frozen/)
remains.

## License

Dual-licensed as `MIT OR Apache-2.0`. Material derived from `td`/`gotd` retains its MIT
notice in [`NOTICE`](NOTICE).
