# rtd

A from-scratch Rust implementation of the [MTProto 2.0](https://core.telegram.org/mtproto)
protocol core and a multi-datacenter, Telegram-compatible server runtime.

`rtd` is a clean-room, wire-compatible reimplementation of the server-facing core of
[`td`](https://github.com/iamxvbaba/td) (a fork of `gotd/td`), built so that a
Telegram-compatible server can support **exact multi-layer operation** and **multiple
datacenters** from the start.

> **Status: in progress.** The sans-IO protocol core (serialization, crypto, framing,
> transports, injectable clock/randomness, the Go cross-check harness, and the generated
> `rtd-tg`/`rtd-mt` value and service models), the multi-layer admission/routing runtime
> (`rtd-tlprofile`), the server edge/RPC runtime (`rtd-server`, `rtd-edge`), and a client
> runtime (`rtd-client`) that completes the auth-key exchange and invokes canonical methods
> are implemented. Multi-DC routing is not started. See [`openspec/`](openspec/) for the plan.

## Progress

| Stage | OpenSpec change | Status |
|---|---|---|
| Foundation — protocol core, generator, manifests | `establish-rtd-foundation` | done (archived) |
| Server runtime — edge, RPC dispatch, in-process core | `add-server-runtime` | done (archived) |
| Multi-layer — exact-profile routing, admission, conversion hooks | `complete-multi-layer` | done (archived) |
| Multi-layer — legacy `ProfileOnly` constructors | `add-legacy-layer-constructors` | done (archived) |
| Multi-DC — routing, migration, authorization transfer, delivery | planned | not started |
| Client library — MTProto 2.0 client runtime | `add-client-runtime` | done (archived) |
| Client library — session persistence | `add-client-session-persistence` | done (archived) |
| Client library — async façade | `add-client-async` | done (archived) |
| Client library — authorization | `add-client-authorization` | done (archived) |
| Client library — updates | `add-client-updates` | done (archived) |
| Server — gramsrv v2 core-exec credential and container | `add-edge-coreexec-token` | done (archived) |
| Server — gramsrv v2 dispatch parity (binding, layer, wire proof) | `add-edge-coreexec-binding` | done (archived) |
| Server — edge resilience (listener, lazy core) | `add-edge-resilience` | done (archived) |
| Server — same-port server descriptor (`/owpengram/server-info`) | `add-edge-server-info` | in progress |

`rtd-edge` runs as a **drop-in container in a gramsrv v2 stack** (postgres, redis, migrate,
file, core): it presents the core-exec bearer token, completes the authorization-key exchange,
persists keys, and dispatches admitted calls. Verified end to end — `rtd-client` through
`rtd-edge` to the prebuilt `core:v2` completes `help.getConfig`, `auth.sendCode` and
`auth.signIn` at layer 229 (the core pins `td v1.7.0`, canonical 229; rtd's canonical is 232).

Layers 225–232 are enabled and the generated sidecar executes direct, retag and rewrite
routes plus every policy conversion: the mechanical classes, the present-in-both hooks, the
nested legacy keyboard-rows adapter (`replyInlineMarkup.rows` on 225–228), and the legacy
`ProfileOnly` button constructors (decoded into the canonical class through the reviewed
`old-only` hooks). Only `inputPasskeyCredentialFirebasePNV` (no hook) fails closed. See
[`openspec/specs/`](openspec/specs/) for the capability specs and
[`openspec/changes/archive/`](openspec/changes/archive/) for the completed changes.

## Goals

- **Wire-compatible** with real Telegram clients and with Go `td`.
- **Multi-layer** by design: one canonical value model plus a generated static
  per-`(profile, wire-id)` routing table, exact-profile admission, and fail-closed
  handling of unsupported layers.
- **Multi-DC**: a server runtime with DC-aware routing, migration, authorization
  transfer, and durable cross-DC delivery.
- **Sans-IO core**: protocol crates own no sockets or runtime and take injected time and
  randomness, so the protocol is deterministic and testable without a network.

## Crate layout

Core (sans-IO, no runtime, no I/O):

| Crate | Role |
|---|---|
| `rtd-bin` | TL binary serialization primitives |
| `rtd-crypto` | MTProto cryptography (AES-IGE, KDF v1/v2, RSA_PAD, DH/PQ, SRP) |
| `rtd-proto` | Message framing, codecs, service primitives |
| `rtd-transport` | obfuscated2 and FakeTLS framing |
| `rtd-mt` | generated MTProto service schema |
| `rtd-tg` | generated canonical Telegram value model |
| `rtd-tlprofile` | generated multi-layer routing + admission runtime |
| `rtd-error` | Telegram TL error parsing |
| `rtd-util` | injectable clock and randomness |
| `rtd-exchange` | authorization-key exchange (client and server flows) |
| `rtd-client` | MTProto 2.0 client runtime (sync core + tokio async façade) |
| `rtd-tmap` | wire-id to name registry |

Runtime and tooling:

| Crate | Role |
|---|---|
| `rtd-server` | server edge, RPC dispatch, multi-DC runtime |
| `rtd-edge-proto` | gRPC contract between the edge and a core service |
| `rtd-edge` | the sidecar edge service binary |
| `rtd-layerhooks-registry` | shared hook-signature registry (generator + hooks) |
| `rtd-layerhooks` | hand-written per-layer conversion hooks the sidecar calls |
| `rtd-codegen` | the multi-layer TL code generator (tool) |
| `rtd` | facade re-exporting the public surface |

## Build and test

```console
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Regenerate the committed generated sources (must produce no diff):

```console
cargo run -p rtd-codegen -- generate
git diff --exit-code
```

### Cross-check vectors

`interop/vectors` is a small Go program that emits protocol vectors from the reference Go
implementation; the Rust tests consume the committed `interop/vectors.json`.

```console
cd interop/vectors
go run . > ../vectors.json
```

### External-peer gate

The official Rust engine (`TelegramMessenger/mtproto-engine`) is used as an
external test peer — its published `mtproto-testserver` binary is run locally, and
rtd must complete the auth-key exchange against it over a real obfuscated2/TCP
socket. The engine is never linked or vendored (it is GPL-2.0).

```console
cargo build --release -p mtproto-testserver --manifest-path path/to/mtproto-engine/Cargo.toml
RTD_PEER_TESTSERVER=path/to/mtproto-engine/target/release/mtproto-testserver cargo test -p rtd-exchange --test peer_gate
RTD_PEER_TESTSERVER=path/to/mtproto-engine/target/release/mtproto-testserver cargo test -p rtd-client --test peer_gate
```

Without `RTD_PEER_TESTSERVER` the gate is skipped; the local `127.0.0.1` peer
needs no VPN.

## Edge sidecar

`rtd-edge` is the runnable MTProto edge. It reads its configuration from
command-line flags or the matching environment variables and serves until it
receives `Ctrl-C`:

| Flag | Environment | Purpose | Default |
|---|---|---|---|
| `--bind <ADDR>` | `RTD_EDGE_BIND` | Listen address | `0.0.0.0:443` |
| `--dc <ID>` | `RTD_EDGE_DC` | Serving datacenter id | `2` |
| `--rsa-key <PATH>` | `RTD_EDGE_RSA_KEY` | PEM RSA private key for the key exchange | required |
| `--secret <HEX>` | `RTD_EDGE_SECRET` | MTProxy secret (plain, `dd`, or `ee`) | none |
| `--websocket` | `RTD_EDGE_WEBSOCKET` | Enable WebSocket ingress | off |
| `--core <ENDPOINT>` | `RTD_EDGE_CORE` | Core `CoreExecService` gRPC endpoint | none |

With no `--core`, the edge runs against the in-process core and its built-in
methods:

```console
cargo run -p rtd-edge -- --bind 127.0.0.1:8443 --dc 2 --rsa-key server.pem
```

To front a remote core, point `--core` at its gRPC endpoint:

```console
cargo run -p rtd-edge -- --bind 0.0.0.0:443 --dc 2 --rsa-key server.pem --core http://127.0.0.1:50051
```

The server-side key-exchange parameters currently come from `rtd-exchange`'s
fixed development `TestServerRng`; a production parameter generator is a
follow-up.

## Workspace conventions

- Edition 2024; the minimum supported Rust version tracks the latest stable toolchain.
- `unsafe` code is forbidden workspace-wide.
- Public items are documented; clippy warnings are denied.
- Generated crates are produced by `rtd-codegen` and never edited by hand.
- `NEXT_SESSION.md` is a local, gitignored session handoff; it is not part of the
  repository.

## Compatibility targets

- [`td`](https://github.com/iamxvbaba/td) — the Go protocol library this reimplements.
- [`TelegramMessenger/mtproto-engine`](https://github.com/TelegramMessenger/mtproto-engine)
  — the official Rust MTProto engine; a **read-only** compatibility target and the
  external peer for interop tests. It is GPL-2.0, so no source is copied and no
  dependency is taken.

## License

Dual-licensed as `MIT OR Apache-2.0`. Material derived from `td`/`gotd` retains its MIT
notice in [`NOTICE`](NOTICE).

## Planning

Plans, specs and decisions live under [`openspec/`](openspec/). See
[`openspec/config.yaml`](openspec/config.yaml) for the project's frozen core decisions.
