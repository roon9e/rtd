# Design

## Context

See `proposal.md` — Why. What already exists and shapes the approach:

- The sans-IO core is complete: `rtd-bin` (TL primitives), `rtd-crypto` (AES-IGE, KDF
  v1/v2, RSA_PAD, DH/PQ, SRP, temp keys), `rtd-proto` (message ids, replay window,
  containers, bounded gzip, rpc_result, **plaintext** messages, transport codecs),
  `rtd-transport` (obfuscated2 + FakeTLS), `rtd-error` (TL errors), `rtd-util`
  (injected clock/randomness), and `rtd-exchange` (both sides of the key exchange).
- `rtd-mt` is the generated MTProto service schema (`req_pq`, `resPQ`, `rpc_result`,
  `msgs_ack`, `ping`/`pong`, `new_session_created`, `bad_server_salt`, …);
  `rtd-tg`/`rtd-tlprofile` are the generated canonical model and multi-layer routing
  runtime (`Profile`, `resolve_profile`, admission, per-profile encode, freeze).
- `rtd-server`, `rtd`, and the `rtd` facade are empty skeletons; `rtd-edge-proto` holds
  the committed prost types (`Hello`, `SessionIdentity`, `AdmittedCall`, `CallResult`,
  `CallError`) and a version handshake; `rtd-edge` only reports/validates a version.
- Frozen decisions that bind this change: D6 (tokio actor, single writer, no locks on the
  hot path), D12 (storage traits now, Postgres later), D14 (sidecar edge over gRPC), D18
  (sans-IO core, IO only in `rtd-server`/`rtd-edge`), and D7 (one TL error type). The
  upstream contract is Go `td`/`gramsrv` and the official `mtproto-engine` (read-only).

Everything the edge must do is already a requirement in `server-edge`, `server-rpc`, and
`edge-service`; this change implements them and the deltas in `specs/` add what those
specs left open.

## Goals / Non-Goals

**Goals**

- A runnable, multi-layer MTProto edge (`rtd-edge`) and the `rtd-server` runtime behind
  it: encrypted sessions, every supported ingress transport, RPC dispatch with layer
  binding, and per-profile result encoding.
- The encrypted session state machine stays **sans-IO and deterministically testable**;
  only the connection actor owns sockets, timers and tasks.
- A gRPC boundary shaped so Go `gramsrv` can adopt rtd as a sidecar **without** an
  in-process link, and so the edge runs with an in-process core for tests.
- Storage behind traits with an in-memory backend, so multi-DC can extend it later.

**Non-Goals**

- No multi-DC behavior (registry, migration errors, authorization transfer, cross-DC
  delivery) — that is stage 4. This change keeps keys/sessions `(dc, key_id)`-scoped so
  stage 4 slots in, but implements exactly one serving DC.
- No layers 225–231 and no non-direct route bodies — that is the parallel generator
  stage; this change runs on canonical layer 232 with the "other layers will be added
  through the same path" assumption.
- No business logic beyond the minimal built-ins; full auth/signup is not here.
- No PostgreSQL/Redis backend, no durable outbox, no updates/difference engine.
- No client library.

## Decisions

### D19. Encrypted framing in `rtd-proto`; the session state machine stays sans-IO

The MTProto 2.0 encrypted envelope (auth-key id, message key, AES-IGE payload) is byte
layout and belongs with the other framing in `rtd-proto` as a new `encrypted` module that
uses `rtd-crypto`'s KDF and IGE.

Message-id/sequence allocation, the replay window, salt tracking, pending-ack and
resend bookkeeping, and session-id ownership are protocol logic with no I/O, so they also
live in the sans-IO core (a `session` module), driven by the injected clock/randomness.
The **connection actor** in `rtd-server` owns the tokio task, reads bytes, and asks the
session state machine what to do.

Alternatives: putting the session machine in `rtd-server` (rejected — makes it
untestable without a runtime and breaks D18's boundary); a new `rtd-session` crate
(rejected — a thin crate for one producer/consumer pair; `rtd-proto` already owns
message ids, replay, and framing, so it is the cohesive home). If a second consumer
(e.g. the deferred client library) appears, extracting a crate is mechanical.

### D20. One tokio actor per connection with a single writer

Each accepted connection becomes a task that owns the socket, the session state machine
and the transport codec. A dedicated writer task (fed by a bounded channel) is the only
writer: it assigns message ids/sequence numbers and serializes outbound frames, so there
are no locks on the hot path (D6). Inbound is the actor's read loop; outbound from RPC
handlers flows through the channel. Timers (ping intervals, resend, session timeouts) run
as `tokio::time` on the actor.

Alternatives: `Arc<Mutex<Session>>` shared across tasks (rejected — locks on the hot path
and races between id allocation and write ordering); a single task doing read+write with
`select!` (viable, but a separate writer keeps slow client writes from stalling reads and
matches the reference servers' outbound actor).

### D21. Ingress is a trait; detection selects the wrapping

Accepting a connection is expressed as one abstraction that yields an owned, framed
MTProto byte stream plus metadata (codec kind, DC label, proxy user). Implementations:
plain TCP with framing detection (abridged / intermediate / padded-intermediate / full,
reusing `rtd_proto::detect_codec`), obfuscated2, FakeTLS/MTProxy (secret-authenticated,
reusing `rtd-transport`), and WebSocket (a WS server that carries the MTProto stream).
The accept loop asks the ingress for the next connection, so the actor is identical
regardless of transport.

Alternatives: an `enum` of transports (rejected — every new transport edits the match);
one accept loop per transport wired independently (rejected — duplicates the actor
setup and admission accounting).

### D22. Storage traits now, in-memory backend, `(dc, key_id)` scoping

`rtd-server` defines `AuthKeyStore` and `SessionStore` traits (store/get/remove/expire)
and ships an in-memory implementation. Keys and sessions are keyed by datacenter and
identifier so stage 4 does not reshape call sites. A store failure fails the operation
closed (never "assume written"), which is the observable requirement in `storage`.

Alternatives: concrete hash maps (rejected — couples the edge to one backend and stalls
the Postgres stage); `async_trait` on the traits (rejected — native `async fn` in traits
is stable and the workspace forbids unjustified dependencies/lints).

### D23. The core boundary is a trait; gRPC is one adapter

The edge dispatches admitted calls through a `Core` trait
(`execute(session, admitted-call) -> CallResult`). Two implementations ship: an
in-process core (used by the built-in router and by tests) and a tonic gRPC adapter that
**dials the core** and calls `EdgeCore.Execute`, reusing `rtd-edge-proto`. The contract
direction is edge → core (the edge runs the client side of `Execute`); lifecycle/routing
events are surfaced as a server-streaming RPC on the same contract. Core
unreachability becomes a TL error to the client; the session stays alive.

Alternatives: the edge hosting the service and the core dialing in (rejected — inverts
the request path: the edge is the one that must call out per request, and the core is the
long-lived business process); defining the boundary only in-process (rejected — the whole
point is that Go `gramsrv` adopts it over the wire); byte-level HTTP/JSON (rejected —
`gramsrv` v2's boundary is gRPC, and prost types already exist).

### D24. Router trait and minimal built-ins

The set of implemented methods is a `Router` the embedder supplies; the built-ins
(`help.getConfig`, `help.getNearestDc`) are one implementation, and a "default" router
answers unknown methods per the `server-rpc` unknown-method behavior. Wrapper unwrapping,
layer binding, result-identity binding and error mapping reuse the existing `rtd-tlprofile`
admission and `rtd-error` parsing.

Alternatives: a generated exhaustive handler enum now (deferred — the method set is still
small and the generator emits dispatch tables when the built-in surface grows); free
functions per method with a global registry (rejected — a trait keeps the edge free of
globals and lets the gRPC adapter implement the same shape).

### D25. Per-profile encoding reuses the multi-layer runtime

The edge binds each session's layer (explicit selection or inherited from the auth key),
corrects it in client message-id order, and encodes every result/update through
`rtd-tlprofile`, failing closed for an unsupported or unrepresentable value. No new
multi-layer machinery is introduced here.

### D26. Verification: deterministic unit tests, in-process e2e, extended peer gate

- The session state machine and encrypt/decrypt paths are unit-tested with
  `ManualClock`/`DeterministicRandom`, including replay, resend and salt-update cases.
- An in-process end-to-end test runs a test client against a spawned rtd edge (ephemeral
  port) across each ingress transport, using a fake core, asserting a full
  exchange → encrypted RPC → result.
- The official-engine **external-peer gate** is extended from handshake-only to a full
  encrypted session: key exchange, then an encrypted `ping` answered by a `pong`. The
  engine remains run-only (never linked/vendored).

### D27. Runtime dependencies are confined to the runtime crates

`rtd-server`/`rtd-edge` add `tokio` and `tonic`/`prost`; WebSocket ingress adds
`tokio-tungstenite`. The sans-IO crates' dependency sets are unchanged, keeping the core
deterministic and light, and keeping the GPL engine off every dependency path.

## Risks / Trade-offs

- [Sequence/message-id or framing divergence breaks real clients] → The peer gate and the
  in-process e2e run the real codecs; the Go `td` vectors still pin ids/framing, and the
  official engine is the external arbiter.
- [WebSocket and MTProxy widen the attack surface and the dependency set] → Keep them
  behind the `Ingress` trait, bound admission budgets before unwrapping, and confine deps
  to runtime crates (D27).
- [Actor model deadlocks or unbounded mailboxes under load] → Bounded outbound channel
  with backpressure, no cross-actor locks, and admission budgets that shed load before
  queues grow.
- [gRPC contract ossifies] → The contract stays byte-oriented and versioned (D14); the
  committed prost types are the only coupling, and version mismatch is rejected at
  handshake.
- [Storage trait shape is wrong for Postgres later] → Keep it minimal and
  `(dc, key_id)`-scoped; the in-memory backend is swappable without touching call sites.
- [Two source-of-truth messages (plaintext vs encrypted) drift] → Share the message-id,
  container and salt code; only the envelope differs, and both are covered by goldens.

## Migration Plan

Additive and opt-in, mirroring stage 3 of the foundation's migration plan:

1. Land the sans-IO encrypted/session modules and their tests (no runtime impact).
2. Land `rtd-server` behind the edge binary and wire `rtd-edge` to run it with an
   in-process core; the gRPC adapter follows.
3. Extend the peer gate; keep the workspace gate green.

Rollback is per-step: the encrypted/session modules are unused until the edge is enabled,
and the edge is a new binary — disabling it restores the previous behavior. No existing
public behavior changes, and no Go `gramsrv` change is required.

## Open Questions

- WebSocket subprotocol/path and whether LAN-only WS is enabled by default — answerable at
  implementation time without changing the specs.
- Whether lifecycle/routing events ship as one server-streaming RPC or a small set of
  unary calls — the contract version can absorb either without affecting the edge.
- Whether to reuse `rtd-exchange`'s `Stream` trait for the handshake inside the edge or
  drive it from the same session state machine — a wiring choice, not a behavior change.
