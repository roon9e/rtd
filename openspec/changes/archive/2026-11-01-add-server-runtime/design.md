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
  hot path), D12 (storage traits — for keys/evidence, superseded here by D22), D14 (sidecar
edge over gRPC), D18
  (sans-IO core, IO only in `rtd-server`/`rtd-edge`), and D7 (one TL error type). The
  upstream contract is Go `td`/`gramsrv` and the official `mtproto-engine` (read-only).

Everything the edge must do is already a requirement in `server-edge`, `server-rpc`, and
`edge-service`; this change implements them and the deltas in `specs/` add what those
specs left open.

An audit of the **already-committed** crates against what the gramsrv edge depends on
(`internal/mtprotoedge` on `origin/v2`) found gaps that must be fixed here for the edge to
be a drop-in replacement: the committed `rtd-edge-proto` shape, the `rtd-tlprofile` runtime
surface, MTProxy secret parsing, and the `td/tmap` name registry. D28 records the fixes.

## Goals / Non-Goals

**Goals**

- A runnable, multi-layer MTProto edge (`rtd-edge`) and the `rtd-server` runtime behind
  it: encrypted sessions, every supported ingress transport, RPC dispatch with layer
  binding, and per-profile result encoding.
- The encrypted session state machine stays **sans-IO and deterministically testable**;
  only the connection actor owns sockets, timers and tasks.
- A boundary that mirrors gramsrv v2's `CoreExecService`, so a gramsrv core can front an
  rtd edge unchanged, and so the edge runs with an in-process core for tests.
- State ownership matching gramsrv v2: the core owns durable auth keys and layer evidence;
  the edge holds only transient connection state and a key cache.

**Non-Goals**

- No multi-DC behavior (registry, migration errors, authorization transfer, cross-DC
  delivery) — that is stage 4. This change implements exactly one serving DC; the
  core-owned key/evidence records stay DC-scoped so stage 4 slots in.
- No layers 225–231 and no non-direct route bodies — that is the parallel generator
  stage; this change runs on canonical layer 232 with the "other layers will be added
  through the same path" assumption.
- No business logic beyond the minimal built-ins; full auth/signup is not here.
- No PostgreSQL/Redis backend, no durable outbox, no updates/difference engine.
- No client library.

## Decisions

### D19. Encrypted envelope in `rtd-crypto`; the session state machine stays sans-IO

Both reference implementations separate the encrypted message from the session state
machine, and neither puts either in the IO runtime:

- Go `td` keeps the envelope in `crypto/encrypted_message.go` (`AuthKeyID`, `MsgKey`,
  `EncryptedData`) with `crypto/salt.go` (`ServerSalt(newNonce, serverNonce)`) and
  `crypto/session_id.go`, and keeps the stateful `Session`/`Conn` (session id, salt,
  `ackLoop`, `saltLoop`) in the `mtproto` package.
- The official engine keeps `mtproto-core::message` (encrypt/decrypt + inner header),
  `mtproto-core::msg_id` and `mtproto-core::session` (with `session::salts`,
  `session::dedupe`) in one sans-IO core crate; only `mtproto-engine` owns IO.

Following both: the **envelope** (encrypt/decrypt, message key, padding, inner header of
salt/session id/message id/sequence) goes in **`rtd-crypto`**, beside the KDF, AES-IGE and
auth key it needs, mirroring `td`'s `crypto/` (the capability is `protocol-crypto`). The
**session state machine** (message-id/sequence allocation, replay window, salt tracking,
pending-acks, resend queue, session identity) goes in the sans-IO core as a `session`
module in **`rtd-proto`**, mirroring the engine's `mtproto-core::session`; `rtd-proto`
gains a dependency on `rtd-crypto` (the engine keeps message and session in a single core
crate; our crate split is only finer-grained). The **connection actor** in `rtd-server`
owns the tokio task, reads bytes, and drives the session machine; it never reimplements
protocol state.

Alternatives: the envelope in `rtd-proto` (workable, but diverges from `td` and pushes a
purely cryptographic operation into the framer); the session machine in `rtd-server`
(rejected — untestable without a runtime and contrary to both references); a new
`rtd-session` crate (rejected — the engine keeps session in its core crate and our core is
`rtd-proto`; extraction stays mechanical if a second consumer appears).

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

### D22. Durable state ownership mirrors gramsrv v2: the core owns keys, the edge is transient

gramsrv v2's `CoreExecService` owns the durable auth keys (`SaveAuthKey`/`GetAuthKey`) and
the durable session-layer evidence (`ResolveNegotiatedSessionLayer`, `PublishLayerEvidence`,
…); the edge keeps only transient per-connection state plus a Redis session registry. rtd
follows that split: the edge keeps an in-memory **session registry** and an **auth-key
cache** for the life of a connection, saves a newly created key to the core before
confirming the exchange, and resolves a key or layer evidence it does not hold by calling
the core. The core is the source of truth; the edge holds no durable store of its own. A
core failure fails the operation closed rather than assuming state was written.

This overturns the archived decision that "rtd owns auth keys/sessions through storage
traits" (D12/D14). It is a deliberate reconciliation with the now-readable `v2` branch on
`origin`.

Alternatives: keep edge-owned storage traits (rejected — diverges from v2, so a gramsrv
core adopting rtd would have to give up its own key store); split ownership where the edge
persists keys and the core persists evidence (rejected — two owners of one auth-key record
for no benefit).

### D23. The core boundary is a trait; gRPC mirrors gramsrv v2's `CoreExecService`

The edge dispatches through a `Core` trait, and two implementations ship: an in-process
core (used by the built-in router and by tests) and a tonic gRPC adapter. This now matches
gramsrv v2's real boundary, read from the public `origin/v2` branch:

- the **edge** holds a `CoreExecService` *client*; the **core** *implements*
  `CoreExecService` (`RegisterCoreExecServiceServer`);
- **admission runs in the edge**: `GRPCRemote.AdmitLayer` calls the edge-side admitter
  locally, and only `DispatchAdmitted` crosses the wire as `DispatchRequest`;
- the core returns `DispatchResponse{ result_wire, rpc_error, post_response_actions }`,
  which the edge encodes per the session's layer.

`rtd-edge-proto` is therefore reshaped toward that contract: the `CoreExecService` name, a
`DispatchRequest`/`DispatchResponse` pair, the durable-state messages (auth keys, layer
evidence, session offline) and an explicit protocol version — so a gramsrv core can front an
rtd edge unchanged. Core unreachability becomes a TL error to the client; the session stays
alive.

Alternatives: the edge hosting the service and the core dialing in (rejected — v2 puts the
client in the edge and the server in the core); a bespoke minimal contract (rejected — it
would force gramsrv to bridge two shapes); byte-level HTTP/JSON (rejected — v2's boundary is
gRPC and prost types already exist).

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

### D28. Aligning already-committed crates with the drop-in target

An audit against gramsrv v2's edge dependencies (the `td` packages used by
`internal/mtprotoedge`) found committed work that is not yet drop-in compatible, fixed in
this change:

- **`rtd-edge-proto`** is committed as `EdgeCore.Hello/Execute`; it must mirror gramsrv
  v2's `internal/coreexec/coreexecpb/coreexec.proto`. gRPC method paths are
  `/<package>.<service>/<method>`, so drop-in requires the same package
  (`telesrv.coreexec.v1`), service (`CoreExecService`), method names and field tags; we
  reproduce the subset the edge uses. gramsrv is the project's own service, so reproducing
  its contract schema is a deliberate choice, recorded here rather than an accidental copy.
- **`rtd-tlprofile`** exposes admission (frozen/default/unprofiled), route lookup, semantic
  mapping, `FrozenObject` freeze/encode and a semantic `Dispatcher`, but the edge RPC layer
  also needs the wrapper surface: initialization-wrapper client/connection info, the
  invoke-after ordering identifiers, a call identity, and a dispatch `Result`. The
  generator and runtime are extended for these; existing admission/freeze behavior is
  unchanged.
- **`rtd-transport`** takes the proxy secret as raw bytes but does not parse the human
  MTProxy secret string (mode prefixes `dd`/`ee`); that parsing is added for edge config.
- **`td/tmap`** (a wire-id→name map merged from the tg/mt/proto maps) has no rtd
  equivalent; a combined registry is added over the generated per-crate registries.

Compatibility was verified by reading the drop-in target's source, not assumed.

**Containment.** The gramsrv contract is confined to `rtd-edge-proto` and the `rtd-edge`
binary. Only `rtd-edge` depends on `rtd-edge-proto`; `rtd-server` (where the `Core` trait
lives) and every sans-IO crate stay free of it, and the contract types are never referenced
outside those two crates. So mirroring `CoreExecService` does **not** make rtd incompatible
as a general `td` library: the proto/codec/crypto/exchange/transport/tg/tlprofile crates
remain gramsrv-agnostic, and a consumer using rtd as a `td` replacement never links the
contract. The wrapper/call surface, secret parsing and name-registry fixes mirror `td`'s
own `tlprofile`/`mtproxy`/`tmap`, so they move rtd *toward* a general `td` library, not away
from it.

**Pin, don't fork.** gramsrv's `coreexec.proto` is treated as a pinned input with recorded
provenance (commit + content digest), the same discipline used for `td`'s schemas, so the
boundary cannot drift silently and can be upgraded deliberately. Only the subset the edge
uses is reproduced, and rtd's own `Core` trait remains the seam, so a future core can
satisfy rtd without matching gramsrv.

Alternatives: leave committed crates as-is and adapt inside `rtd-server` (rejected — the
edge-proto shape cannot be adapted away, and the runtime gap would leak Go-shaped
workarounds into the edge); copy gramsrv's packages wholesale (rejected — only the boundary
schema is mirrored, only the subset in use, and only behind the integration crates); define
a new generic edge contract instead of gramsrv's (deferred — it would not be drop-in with
the existing gramsrv core, which is the point of D14; the `Core` trait keeps it possible).

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
- [Key/evidence ownership diverges from gramsrv v2] → Mirror v2's `CoreExecService`
  (D22/D23): the core owns durable state and the edge holds only transient state, so a
  gramsrv core adopts rtd with no change to its key store.
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
  unary calls — gramsrv v2 uses unary calls (`ReportSessionOffline`, the layer-evidence
  RPCs), so the default is unary; a streaming variant could be added without affecting the
  edge.
- Whether to reuse `rtd-exchange`'s `Stream` trait for the handshake inside the edge or
  drive it from the same session state machine — a wiring choice, not a behavior change.
