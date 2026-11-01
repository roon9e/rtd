# Design

## Context

See `proposal.md` for motivation. The relevant current state:

- `td` (Go, `github.com/iamxvbaba/td`) is a fork of `gotd/td` at canonical **Layer 232**.
  Its multi-layer support is a generated sidecar, `tlprofile`, built on a
  "sparse AOT" model: one canonical value model plus a **static** per-`(profile, wire-id)`
  routing table over eight profiles (225–232). Routes are classified `direct`, `retag`,
  `rewrite`, `policy`, `profile-only` or `unavailable`; plans are deduplicated by content
  digest; unknown layers **fail closed**; there is no runtime schema and no byte↔byte
  transcoder.
- `gramsrv` (Go, module `telesrv`) consumes the server-facing subset of `td`:
  `bin`, `proto`/`proto/codec`, `crypto`, `exchange`, `mt`, `transport`,
  `mtproxy/obfuscated2`, `tgerr`, `tmap`, `tg`, and — decisively — `tlprofile`.
  It forbids canonical fallback on the result path and encodes results/updates per exact
  profile.
- `gramsrv` is **single-DC**: one configured DC id, media `dc_id` rewritten to the local DC,
  no `MIGRATE` errors, and `StrictDC` is a diagnostic only.

## Goals / Non-Goals

**Goals**

- A Rust workspace that implements the server-facing protocol core of `td` with
  **wire-level compatibility**, verifiable by cross-implementation tests.
- Multi-layer behavior at least as exact as `tlprofile`: exact-profile admission,
  per-profile encoding, fail-closed on unknown layers, stable semantic identity.
- A **new, manifest-driven Rust generator** whose inputs are versioned schemas plus a
  reviewed policy, and whose output is committed.
- A design for a **multi-DC server runtime** (edge + RPC + routing + delivery) that does
  not exist server-side today.
- Foundations that let a client library be added later without rework.

**Non-Goals**

- No client library in this change (`telegram.Client`, updates engine, auth flows,
  downloader/uploader, session-file importers).
- No in-process Go↔Rust FFI/cgo and no rewrite of Go `gramsrv`; integration with
  `gramsrv` is via the sidecar edge service only (D14).
- No runtime schema interpretation, and no canonical↔old transcode bridge.
- No performance tuning beyond keeping the architecture allocation-conscious.

## Decisions

### D1. Represent multi-layer semantics Rust-idiomatically, not as a transliteration

Keep every guarantee of the sparse-AOT design, but change the representation:

- **TL classes become Rust enums**, not `interface + struct-per-constructor + type switch`.
  A boxed TL object is `Box<TlObject>` where `TlObject` is a generated enum; unknown or
  historical constructors get explicit variants carrying the raw payload, never a silent
  canonical decode.
- **Routing becomes generated `match` tables** keyed by `(profile, wire_id)`, which Rust
  compiles to jump tables — the same "static AOT" property as the Go `switch`.
- **Codecs are traits** (`Encode`/`Decode`) implemented by generated code, with
  `EncodeBare`/`DecodeBare` equivalents; no reflection, no dynamic dispatch on the hot path.
- **Buffers are zero-copy**: a cursor over `bytes::Bytes`/`&[u8]` with explicit
  owned-vs-borrowed handling, replacing Go's mutate-and-reslice `bin.Buffer`.
- **Handlers are registered by semantic id** and dispatched through a generated enum,
  giving compile-time exhaustiveness instead of Go's runtime type assertions.

Alternatives: a literal Go transliteration (rejected — fights Rust's ownership model,
forces `dyn` on the hot path, and risks compile-time blowup); a canonical+conversion layer
(rejected — breaks the exact-profile contract `gramsrv` relies on); a runtime schema
(rejected — cannot enforce bounded admission and contradicts the design's purpose).

### D2. Cargo workspace split by domain, with a facade

Crates (grep-friendly, acyclic, generated code isolated for compile times):

| Crate | Contents |
|---|---|
| `rtd-bin` | TL primitives, cursor/buffer, fields bitfield, checked variants |
| `rtd-proto` | message ids + replay window, containers, bounded gzip, rpc result, unencrypted messages, transport codecs |
| `rtd-crypto` | AES-IGE, KDF v1/v2, auth key identity, RSA exchange padding, DH/PQ, SRP, temporary keys |
| `rtd-transport` | TCP/WebSocket connections, obfuscated2, FakeTLS/MTProxy |
| `rtd-mt` | generated MTProto service (`mt`) schema |
| `rtd-tg` | generated canonical value model (layer 232 first) |
| `rtd-tlprofile` | generated profile sidecar + runtime admission/dispatch/freeze |
| `rtd-error` | TL error type and parsing (`tgerr` equivalent) |
| `rtd-util` | injectable clock, sync primitives, ready signals |
| `rtd-server` | edge, RPC dispatch, layer binding, multi-DC runtime |
| `rtd-edge-proto` | protobuf/gRPC contract between the rtd edge and a core service (D14) |
| `rtd-edge` (bin) | the sidecar edge service binary (D14) |
| `rtd` | facade re-exporting the public surface |
| `rtd-codegen` (tool) | the generator; not published |

Alternatives: a single crate (rejected — 100k+ generated lines would make incremental
builds painful); a few coarse crates (rejected — less clear dependency direction, weaker
compile-time isolation of generated code).

### D3. New Rust generator, committed output, digest-deduplicated plans

- The generator is a standalone binary (run via an `xtask`-style command), **not** a
  `build.rs`. Generated Rust is committed, so normal builds never generate.
- Generation-time intermediate representation mirrors the proven one: a **universe** of
  semantic families, each with per-layer `Definition`s sharing **wire codecs** where the
  encoded payload is identical; three digests per definition (body, signature, wire) so
  that non-wire differences (field names, presence-only flags, result types) do not
  duplicate codecs.
- **Route classification** is computed for the full `(wire × profile)` product and every
  pair must receive an explicit action; pairs with no action are a generation error, not a
  guess.
- **Policy** resolves non-mechanical differences. Each obligation gets a domain-separated
  SHA-256 key; generation fails on unreviewed differences and on stale policy entries.
- **Plans** (body, preflight, result) are deduplicated by digest so identical behavior
  across profiles/wire ids is emitted once.
- Output is written **atomically and deterministically**; a declared size/line budget is
  enforced for the profile sidecar.

### D4. Hybrid schema source: bootstrap from `td`, own it going forward

Import `td`'s `_schema/layers/*.tl`, `manifest.json` and `policy.json` into rtd once,
preserving each schema's original tdesktop repository/commit/blob/SHA-256 provenance
(which the manifest already carries). rtd then owns `policy.json`. The import tool is
built so a schema can later be fetched from **either** `td` or `tdesktop` directly without
changing the generator. Explicit constructor identifiers are required on every line.

Alternatives: treating `td` as a permanent subtree (rejected — couples rtd's release to
`td`); re-deriving and re-authoring policy from scratch (rejected — discards audited
decisions for no benefit).

### D5. Multi-DC runtime (the genuinely new subsystem)

- **DC registry**: a configured set of datacenters, each with id, address(es), RSA key and
  fingerprint, and flags (media-only, CDN, test). `help.getConfig` advertises all of them.
- **Per-DC keys**: the key exchange uses the serving DC's key; fingerprints are
  DC-specific.
- **DC-aware storage**: auth keys and sessions are owned by the DC that issued them;
  lookups are `(dc, key_id)`-scoped.
- **Routing**: a request carrying an auth key not owned by the local DC yields either a
  migration error (client retargets) or an authorization transfer, per policy.
- **Migration errors**: `PHONE_MIGRATE_<dc>`, `NETWORK_MIGRATE_<dc>`, `USER_MIGRATE_<dc>`,
  `FILE_MIGRATE_<dc>` are produced from the error layer and validated against the registry.
- **Media/webfile routing**: file references name the storing DC, not the local one.
- **Authorization transfer**: an export/import token scheme (server-side equivalent of
  `auth.exportAuthorization`/`importAuthorization`) signed with a per-deployment secret.
- **Cross-DC delivery**: kept behind an abstraction so the transport can be chosen later
  (see Open Questions). Session registry and delivery are expressed as interfaces that a
  single-DC or multi-DC implementation can satisfy.
- **Label validation** is a policy switch (strict vs permissive), replacing `gramsrv`'s
  diagnostic-only `StrictDC`.

### D6. Runtime and concurrency

- **tokio** multi-threaded runtime. Each connection is an **actor**: exactly one task owns
  it and a dedicated single writer assigns message ids/sequence numbers and serializes
  writes, so there are no locks on the hot path (equivalent to `gramsrv`'s per-connection
  outbound actor and to the official engine's session ownership model).
- Admission budgets via semaphores/token accounting (total connections, per-source,
  concurrent handshakes, aggregate frame bytes).
- No blocking calls on the runtime; file/DB work is deferred to spawned blocking tasks.
- Time and randomness are injected into the sans-IO core (D18), so message-id, salt and
  timeout behavior is testable deterministically.
- tokio is kept over a hand-rolled mio reactor because the edge boundary needs gRPC/tonic
  (D14); the actor model supplies the no-lock property a reactor approach would give.

### D7. Error model

One TL error type carrying an integer code and message, with the numeric-suffix parse
(`TYPE_<n>` → type + argument) and `FLOOD_WAIT` handling replicated exactly. Internal
failures use a Rust error enum; the TL error is the wire-visible projection.

### D8. Edition, MSRV, license

Edition 2024; `rust-version` tracks the latest stable toolchain (no back-compat burden).
License `MIT OR Apache-2.0`; `td`/`gotd` MIT notices retained in a `NOTICE`/`THIRD-PARTY`
file for derived material.

### D9. Multi-DC state ownership: a central shared store

Account, chat and message state lives in one authoritative store (PostgreSQL, with Redis
for ephemeral state such as presence and location) that all datacenters share. Datacenters
act as edge replicas over the same state. This matches `gramsrv` v2 and lets multi-DC
routing work without any replication protocol.

Storage is accessed only through traits, so per-DC sharding (Telegram's "home DC" model)
remains a possible later evolution without changing call sites.

Alternatives: per-DC shards (rejected for now — requires a home-DC concept, replication,
conflict handling and cross-DC forwarding, a large amount of new design for no immediate
benefit to a self-hosted deployment).

### D10. Cross-DC delivery: a `Delivery` trait, durable outbox first

Delivery is expressed as an abstraction. The first implementation is **single-process**
(in-process channels across datacenters in one process); the first *distributed*
implementation is a **durable outbox**: a producer writes a durable row, any datacenter's
delivery worker claims and leases it, projects it per session, and acknowledges it
(the claim/project/ACK shape `gramsrv` v2 uses). Mesh and broker backends remain possible
later.

Guarantees: **at-least-once** delivery with **idempotent per-session projection** and
**per-chat ordering**. Exactly-once is not promised at the transport layer; idempotence
makes at-least-once safe.

Alternatives: commit directly to the outbox with no abstraction (rejected — blocks
single-process dev and future backends); direct DC-to-DC RPC mesh (rejected for now —
discovery, retry, dedup and ordering all hand-rolled); external broker (rejected for now —
new infra and ops for a small number of datacenters).

### D11. Canonical-layer bump policy

Regeneration mechanics are automated (one command), but advancing the canonical layer is
accepted only as **its own reviewed change** that must include a checked-in decision record
and green cross-implementation tests. A canonical bump is never automatic on import, and
non-canonical layers remain purely additive profiles.

Alternatives: automatic on import (rejected — un-reviewed core-model churn contradicts the
fail-closed philosophy); pinning canonical forever (rejected — the canonical model would
drift behind reality).

### D12. Persistence scope

`rtd` defines **storage traits** (auth keys, sessions, negotiated-layer evidence, delivery
outbox) and ships **in-memory** implementations used by tests and the single-process
runtime. A **first-party PostgreSQL backend** is added at the multi-DC/outbox stage, when a
durable store is actually required. Consumers may always supply their own backend.

Alternatives: in-memory only, ever (rejected — a ready-to-run server needs persistence);
Postgres now (rejected — pulls DB concerns into the foundation before the outbox needs
them); embedded store (rejected — unsuitable for multi-host scale).

### D13. VCS, branches, and CI

Development happens on the `dev` branch; releases are merged to `main`. Commits are made as
`roon9e <roon9e@users.noreply.github.com>`; the GitHub account is `roon9e`. CI runs
**independent checks** — format, clippy, build, workspace tests, interop, and a
generation-freshness check that re-running the generator produces no diff — on both `dev`
and `main`, mirroring `td`'s approach of verifying generated artifacts in CI.

### D14. `gramsrv` integration: sidecar edge service over gRPC

rtd is usable from Go `gramsrv` as a **sidecar MTProto edge**. The boundary is gRPC and
carries raw TL bytes plus admitted-call metadata, mirroring `gramsrv` v2's Edge→Core
boundary ("CoreExec gRPC, TL bytes"). rtd owns the **full edge — including multi-layer
admission, wrapper handling and layer binding** — so the Go core does not need
`td/tlprofile` on the hot path.

Boundary responsibilities:

- **Request path**: the core receives `(session, admitted semantic method, negotiated
  profile, wire id, canonical request bytes)`; it runs business logic and returns a result.
- **Result path**: the core returns a canonical value; rtd encodes it **per the session's
  negotiated profile** and writes it — fail-closed, never a canonical fallback.
- **Lifecycle/events**: session created/closed, negotiated-layer changes, DC routing and
  migration decisions are surfaced to the core as events.
- **Keys/sessions**: owned by rtd's edge and its storage traits (D12); the core consults
  them through the boundary rather than holding them.

This adds an `rtd-edge-proto` crate (the protobuf/gRPC contract) and an `rtd-edge` service
binary, and gives milestone 1 a reason to fix the contract shape early.

Alternatives: in-process C ABI/cgo (rejected — pins threads for async work, needs Rust→Go
upcalls, complicates Windows builds); a transport+crypto-only boundary (rejected — leaves
the hard multi-layer work in Go, which is exactly what rtd is being built to own); full Rust
server replacing `gramsrv` (still the long-term direction, but the sidecar lets `gramsrv`
adopt rtd incrementally).

### D15. Multi-DC placement: fixed primary DC and central Core

A single designated **primary DC** handles account login (`auth.sendCode` / `auth.signIn`);
every other DC answers with `PHONE_MIGRATE_<primary>`. There is no per-account home DC —
because state is central (D9), "primary" is a routing policy, not a data partition.

The **Core runs centrally**; the geographically distributed datacenters are **edge-only**
(one or more rtd edge instances each) and talk to the central Core over gRPC. This matches
`gramsrv` v2 and keeps operational complexity low.

Alternatives: per-account home DC (rejected — adds an assignment and routing policy for no
benefit while state is central); Core per DC with read replicas (rejected — more infra, little
gain); Core split by domain (deferred).

### D16. Authorization-transfer signing

The authorization-transfer token (`auth.exportAuthorization` → `auth.importAuthorization`)
is signed with a **shared deployment secret** using an HMAC over `(user id, source DC,
expiry)`. Any DC validates it with the same secret; import mints a new **DC-local auth key**
bound to the user. The secret is rotatable.

Alternatives: asymmetric signing (rejected — per-DC key management for no gain while the DCs
already share a trust domain); store-backed one-time tokens (rejected — extra DB round-trip;
revocation matters less here).

### D17. Official-engine compatibility target (hard gate)

rtd tracks the official Rust engine `TelegramMessenger/mtproto-engine` as a first-class
wire-compatibility target alongside Go `td`. **Interoperability with that engine is an
acceptance requirement for milestone 1**, not merely a goal: the milestone is not accepted
until interop against that engine passes.

The engine is a **client**, so compatibility is demonstrated by cross-implementation interop
(our code against its published `mtproto-testserver`, and its client path against our server)
plus behavioural alignment with its published `docs/protocol-coverage.md`.

**Licensing**: the engine is GPL-2.0, and GPL-2.0-only is incompatible with Apache-2.0. rtd is
MIT OR Apache-2.0, so rtd copies **no** source from it and takes **no** dependency on it: it is
read-only. Running its published binaries as an external test peer is permitted (testing, not
derivative distribution).

Alternatives: depend on `mtproto-core` (rejected — would force rtd and all consumers to GPL and
break the Apache-2.0 half of our dual license); ignore the engine (rejected — it is the closest
authoritative Rust MTProto implementation and the best available interop counterpart).

### D18. Sans-IO protocol core with injected time and randomness

The protocol crates (`rtd-bin`, `rtd-crypto`, `rtd-proto`, `rtd-transport`, `rtd-mt`, `rtd-tg`,
`rtd-tlprofile`) are **sans-IO**: they own no sockets, no runtime, no tasks, and no ambient time
or randomness. They expose byte-in/byte-out codecs and explicit state machines driven by the
caller, and time and randomness enter as injected traits rather than OS calls.

Only the runtime crates (`rtd-server`, `rtd-edge`) own I/O: tokio, sockets, timers, and the gRPC
boundary.

Consequences: the session and RPC state machines are unit-testable with simulated time and
deterministic randomness and are fuzzable without a network, and the same core can later back a
client, a server, or a test harness unchanged.

Alternatives: an IO-coupled core (rejected — untestable without sockets, non-deterministic);
sans-IO for only a subset (rejected — the boundary is cheap and pays off across the whole core).

## Multi-DC architecture

**Mental model: one authoritative state, many edge replicas.** A client speaks MTProto to
whichever DC it is connected to; that DC's rtd edge does all protocol work, but the data
(accounts, chats, messages, the update sequence, auth keys) lives in one shared store. DCs
differ only in where connections land and where media bytes live — not in what they know.
This is why multi-DC is tractable: no replication protocol, no home-DC state routing, no
cross-DC consistency negotiation.

### Components

| Component | Built from | Owns |
|---|---|---|
| DC edge | rtd `rtd-edge` binary | Transport/obfuscation, key exchange, auth keys, sessions, admission budgets, msg-id/seq, service messages, multi-layer admission + layer binding, per-profile result encoding, DC routing, migration errors |
| Core | Go `gramsrv` (later optionally Rust) | Business RPCs invoked over gRPC with TL bytes |
| Delivery worker (Egress) | rtd runtime (stage 4) | Claims the outbox, projects events per session/layer, pushes to edges, consumes client ACKs |
| File service | gramsrv FileData (or rtd later) | Blob storage; file references name the storing DC |
| Store | rtd storage traits → Postgres (+ Redis) | Durable: auth keys, sessions, account/chat/message state, outbox. Ephemeral (Redis): session registry, presence, push channels |

```
                 Clients (Desktop / Android / iOS / Web)
                   │  MTProto (per-DC auth key)
      ┌────────────┼─────────────────────────────┐
      ▼            ▼                              ▼
  ┌────────┐   ┌────────┐                     ┌────────┐
  │ DC1    │   │ DC2    │      ...            │ DCn    │   each DC = rtd edge instances
  │ edge   │   │ edge   │                     │ edge   │   (its own RSA key + DCOption)
  └───┬────┘   └───┬────┘                     └───┬────┘
      │            │                              │
      │ gRPC (TL bytes + session/layer metadata)  │
      └────────────┼──────────────────────────────┘
                   ▼
             ┌───────────┐        ┌───────────┐
             │  Core(s)  │◄──────►│ Egress /  │   claim → project → push → ACK
             │ (gramsrv) │        │ Delivery  │
             └─────┬─────┘        └─────┬─────┘
                   │  writes            │ outbox rows / session registry
                   ▼                    ▼
             ┌─────────────────────────────────┐
             │  Postgres (state + outbox)      │
             │  Redis (location/registry/push) │
             └─────────────────────────────────┘
```

### Storage model

- **Postgres is the source of truth**: auth keys (with DC id, and expiry for temp keys),
  sessions, account/chat/message state, and the durable outbox.
- **Redis is ephemeral**: session registry (`session → {dc, edge, user, layer, pts
  watermark}`), presence/location, push/control channels. Losing Redis is survivable —
  sessions re-register on reconnect and the outbox replays.
- rtd owns auth keys/sessions through storage traits (D12); the Core consults them through the
  boundary rather than holding them (D14).

### Request lifecycles

1. **Connect + handshake.** `help.getConfig` lists all DCs (id, address, RSA key). The client
   handshakes against its chosen DC with *that DC's* RSA key and receives a **DC-local auth
   key** (permanent or PFS temp). It is stored tagged `(dc_id, auth_key_id)`; the session is
   registered in Redis with the holding edge instance.
2. **Login.** Login RPCs are valid only on the primary DC (D15); elsewhere the edge returns
   `PHONE_MIGRATE_<primary>`.
3. **Normal RPC.** The edge admits the request (multi-layer), attaches `(session, user, dc,
   semantic method, negotiated layer, wire id, canonical bytes)` and calls the Core over gRPC.
   The Core runs business logic and returns a **canonical result**; the edge encodes it per the
   session's layer and writes it — fail-closed, never a canonical fallback.
4. **Media/files.** Blobs are owned by a DC; file references carry that `dc_id`. A request for
   bytes hosted elsewhere yields `FILE_MIGRATE_<owner_dc>`.

### Migration and authorization transfer

Clients react to the standard migrate errors: `PHONE_MIGRATE_<n>` (login), `NETWORK_MIGRATE_<n>`
(connection), `USER_MIGRATE_<n>` (account host), `FILE_MIGRATE_<n>` (media host). To act on a
non-primary DC the client calls `auth.exportAuthorization(current_dc)` then
`auth.importAuthorization(id, bytes)` on the target. Export produces a signed, expiring token
(D16); import validates it and mints a new DC-local auth key bound to the user.

### Updates and delivery

Because pts is a **global per-account (and per-channel)** counter in the shared store, the
update sequence is coherent across DCs for free and any DC can serve `updates.getDifference`.

1. **Produce** — the Core commits its state change *and* writes an outbox row in the same
   transaction (`event`, scope, ordering key, pts).
2. **Claim** — an Egress worker leases a batch of rows (lease + expiry, so a dead worker's rows
   are re-claimed).
3. **Project** — for each target session, encode the update for that session's negotiated layer
   (freeze the canonical value once, encode per profile), respecting per-chat ordering.
4. **Push** — look up the session in the Redis registry and push to the holding edge; the edge
   writes to the client.
5. **ACK** — client `msgs_ack` → edge → Egress marks delivered; unacked rows are retransmitted.

Guarantee: **at-least-once with idempotent per-session projection** and **per-chat ordering**
(D10).

### Layer binding in a multi-DC world

Layer is bound per session and by a mutable auth-key default in the shared store, keyed by
`(dc, auth_key)`. Because each DC issues its own auth key, there is no cross-DC layer-sharing
hazard; corrections are ordered by client msg-id and stale per-layer output is dropped.

### Failure and consistency

- Postgres down → the edge cannot persist keys/sessions; fail closed, do not admit new sessions.
- Edge crash → sessions reconnect; auth keys are durable, so a new edge resumes.
- Redis loss → registry rebuilds on reconnect; the outbox replays; no data loss.
- Egress crash → leases expire; rows are re-claimed; at-least-once covers it.

### Dev and rollout

- **Single-process mode**: all DCs in one process, in-memory store, in-process `Delivery`
  channel — the whole system runs with no infrastructure for tests and dev.
- **Rollout from `gramsrv`**: stand up the rtd edge as a sidecar for **one** DC first (protocol
  parity), then add DCs one at a time, then enable migration and durable delivery. The Go Core
  is unchanged throughout (D14).

## Candidate adoptions (deferred)

Surfaced while reviewing the official engine; deliberately not committed because their value is
not yet decided (see the change's decision log):

- **`zeroize` for key material** in `rtd-crypto`: we redact logs but do not wipe key bytes from
  memory.
- **A security-requirement-to-test matrix** (the official engine ships `security/requirements.tsv`)
  mapping each MTProto security guideline to the test that pins it.

## Risks / Trade-offs

- [Rust-idiomatic redesign diverges from Go in a subtle behavior] → Keep the semantic-ID,
  digest and route-classification algorithms identical to Go and port their tests; add
  cross-implementation interop tests as the acceptance gate.
- [Generated code size hurts compile times] → Isolate generated crates (`rtd-tg`,
  `rtd-tlprofile`), enforce the source budget, and keep deduplication by digest.
- [Zero-copy buffers complicate lifetimes in handlers] → Provide an owned `Buffer` escape
  hatch (like Go's `ResetN`/`Copy`) and default generated decoders to borrowing only for
  short-lived payloads.
- [Cross-DC delivery adds distributed-systems complexity] → Keep it behind a `Delivery`
  trait (D10); single-process is a valid first implementation and the durable outbox is
  added at stage 4.
- [`gramsrv` sidecar boundary could ossify the edge API] → Version the gRPC contract
  explicitly and keep it byte-oriented (D14) so it can evolve without breaking the core.
- [Policy bootstrap could import decisions that assume Go internals] → Policy entries are
  semantic (kind + digest over semantic shapes), so they port; any entry that cannot be
  mapped is reported for manual review rather than dropped.
- [Edition 2024 + latest-stable MSRV reduces who can build rtd] → Accepted: the project
  controls its toolchain; revisit if external Rust consumers appear.
- [Accidental GPL contamination from the official engine] → Treat it as read-only (D17): no
  source copied, no dependency taken, and no build path to it; only its published binaries are
  run as a test peer.
- [An external-peer gate could block if the official engine's binaries are unavailable or
  refuse our traffic] → The gate is explicit and reported; failing it names the exact
  divergence rather than being silently skipped.

## Migration Plan

Not applicable in the deployment sense — `rtd` is a new project and this change does not
modify Go `gramsrv`. "Migration" here is staged capability growth:

1. **Foundation (this change):** workspace, `rtd-bin`, `rtd-crypto`, `rtd-proto`,
   generator skeleton proven on layer 232, interop harness skeleton.
2. **Schema & multi-layer runtime:** full canonical model emission, profile sidecar,
   admission/dispatch/freeze, fail-closed proofs, layers 231→225.
3. **Server edge + RPC + sidecar boundary:** connection lifecycle, key exchange, sessions,
   budgets, wrapper handling, layer binding, per-profile result encoding, and the gRPC edge
   contract of D14 so `gramsrv` can adopt rtd as its edge.
4. **Multi-DC:** DC registry/keys/routing, migration errors, authorization transfer,
   cross-DC delivery.
5. **Optional client library** on the same primitives.

Rollback is per-stage: each stage is additive and the generated artifacts are versioned.
