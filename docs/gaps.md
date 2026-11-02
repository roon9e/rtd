# Feature gaps vs `td`

This document records which features of the Go reference implementation
[`td`](https://github.com/iamxvbaba/td) (a fork of `gotd/td`) are **not yet
implemented** in `rtd`.

It is a point-in-time audit (2026-11-02) produced by reading both trees:

- Go reference: `C:\Users\roon9e\Documents\github\td` — canonical layer 231.
- Rust implementation: this repository — canonical layer 232, profiles 225–232.

`rtd` is a clean-room, wire-compatible reimplementation of the **server-facing
core** of `td` plus a multi-DC server runtime; the high-level client library is a
later, secondary goal. So a gap here is not necessarily a defect — see
[Intentional scope decisions](#intentional-scope-decisions).

## Legend

| Symbol | Meaning |
|---|---|
| ✅ | Implemented in `rtd` |
| 🟡 | Partially implemented |
| ❌ | Absent |
| ⛔ | Intentionally out of scope or deferred by a frozen decision |

## Summary

`rtd` has **full parity on the sans-IO protocol core** (serialization, crypto
primitives, framing/codecs, transport obfuscation including all MTProxy
primitives, key exchange, service messages, error/type maps, injected clock and
randomness) and adds things `td` does not have (multi-layer 225–232 routing, a
server edge/RPC runtime, the gRPC Edge↔Core contract).

Everything **above the raw-RPC layer** is the gap. The largest missing areas, in
rough order of size:

1. The entire high-level **client convenience layer** (`telegram/*`: uploader,
   downloader/CDN, message builders, query pagination, peers, takeout, deeplink,
   thumbnail, rich text, …).
2. **Multi-DC server program** — routing, migration, authorization transfer,
   durable cross-DC delivery, PostgreSQL + Redis storage.
3. **Client conformance details** — background receive loop, keepalive/reconnect,
   multi-salt, 2FA/SRP, updates gap recovery, connection pooling, a real
   MTProxy **dialer**.
4. **Server-side updates / pts-qts-seq delivery** and **per-method rate limiting**.

---

## 1. Protocol core (sans-IO)

| `td` area | `rtd` | Status | Notes |
|---|---|---|---|
| `bin` serialization | `rtd-bin` | ✅ | `Writer`, `Reader`, `Fields`, int128/256 |
| `crypto` (AES-IGE, KDF v1/v2, RSA, DH/PQ, PRNG) | `rtd-crypto` | ✅ | incl. `temp_keys`, `bind` |
| `crypto/srp` — **client** hashing/proof | `rtd-crypto::srp` | ✅ | `hash`, `new_hash` |
| `crypto/srp` — **server verifier** | — | ❌ | Explicitly "client side" only; no `srp_B` derivation / M1 verification. Blocks `auth.checkPassword` end-to-end and any server 2FA. |
| `proto` (container, gzip, msg-id, rpc_result, unencrypted) | `rtd-proto` | ✅ | |
| `proto/codec` (abridged/intermediate/padded/full/noheader) | `rtd-proto::codec` | ✅ | incl. `detect_codec` |
| `exchange` (client + server flows, temp/PFS mode) | `rtd-exchange` | ✅/🟡 | Server params come from the fixed `TestServerRng`; production parameter generator is a follow-up. |
| `mtproto/salt.go` + `mtproto/salts` (multi-salt) | `rtd-server` answers `get_future_salts`; client holds a **single** salt | 🟡 | Client multi-salt refresh (`Vec<(valid_until, salt)>`, `future_salts` handling) is absent. |
| `crypto/temp_keys` + `bind`, `mtproto/pfs.go`, `bind.go` | exchange temp mode + `rtd-crypto::bind` | 🟡 | No temp-key rotation/lifecycle in the client. |
| `tgerr` | `rtd-error` | ✅ | incl. `FLOOD_WAIT` parsing/`as_flood_wait` |
| `tmap` | `rtd-tmap` | ✅ | |
| `clock` | `rtd-util::Clock` (`System`/`Manual`) | ✅ | |
| `clock` NTP source | — | ❌ | Only injectable wall clock; no time sync. |
| `constant` (credentials/ids/tdlib ids/upload limits) | constants live inline in crates | 🟡 | No unified `rtd-constant` crate; upload limits not centralized. |
| `syncio`, `tdsync`, `ascii` | — | ⛔ | Go-runtime-specific helpers with no Rust analogue needed. |

## 2. Transport & MTProxy

| `td` area | `rtd` | Status | Notes |
|---|---|---|---|
| TCP transport + codecs | `rtd-proto` + `rtd-client::transport` | ✅ | |
| `mtproxy/obfuscated2` — client `handshake` **and** server `accept` | `rtd-transport::obfuscated2` | ✅ | Golden-vector tested |
| `mtproxy/faketls` — records, ClientHello/ServerHello, HMAC secret binding | `rtd-transport::faketls` | ✅ | |
| `mtproxy` secret parsing (plain / `dd` secured / `ee` TLS + cloak host) | `rtd-transport::secret` | ✅ | |
| `mtproxy/obfuscator` — `Obfuscator` interface + dialers (`Obfuscated2()`, `FakeTLS()`) | only `rtd-client::connect_tcp(addr, dc, codec, obfuscation: Option<&[u8]>)` | 🟡 | **Missing:** a `Dialer`/`Obfuscator` trait, multi-endpoint/proxy selection, and connection setup that composes FakeTLS. |
| FakeTLS selected by the **client** dialer | `connect_tcp` is obfuscated2-only | ❌ | `faketls` exists and the server accepts it, but the client never selects it. |
| `dd`-secured proxy semantics, proxy tags, MTProxy server role | — | ❌ | `rtd` has no MTProxy *server* (proxy that forwards clients to DCs, promoted-channel tags, `dd` proxy-secret mediation). |
| `transport/websocket.go` (server listener) + `dcs/websocket.go` + `wsutil` (client) | server ingress only (`rtd-server`, `tokio-tungstenite`) | 🟡 | Client-side WebSocket transport absent. |
| MTProto-over-HTTP / `http_wait` | — | ❌ | (Also deliberately skipped by the `td` fork as client-only.) |

**MTProxy in one line:** all *transport primitives* are present (obfuscated2
client+server, FakeTLS client+server, secret parsing incl. cloak); the missing
piece is the *client conformance layer* — a dialer/obfuscator abstraction that
composes obfuscated2 + FakeTLS and selects it from a secret.

## 3. High-level client (`telegram`)

| `td` area | `rtd` | Status | Notes |
|---|---|---|---|
| `telegram.Client` lifecycle (`Run`, `API()`, `Invoke`, `Self`, `Ping`) | `rtd-client::Client` / `AsyncClient` | 🟡 | Sync core + tokio façade exist; no `Run`/middleware/`Self`/`Ping`. |
| `Options` (reconnect, PFS, device, tracer, stores, clocks, timeouts) | minimal options | 🟡 | |
| `telegram/auth` user flow (`Flow`, `SendCode`, `SignIn`, `SignUp`, `AcceptTOS`) | `send_code` / `sign_in` | 🟡 | Code-only path only. |
| 2FA / SRP (`auth.checkPassword`, `Password`, `PasswordWith`) | — | ❌ | Needs the server-side SRP verifier (see §1). |
| `auth/bot.go` bot-token login (`Auth().Bot`) | — | ❌ | |
| `auth/qrlogin` (export/accept/import QR login) | — | ❌ | |
| `auth/srpguard` (memguard password handling) | — | ❌ | |
| password recovery (`RequestPasswordRecovery`/`CheckRecoveryPassword`/`RecoverPassword`) | — | ❌ | |
| `telegram/updates` manager (pts/qts/seq gaps, `getDifference`, channel state, access hashes) | basic `Updates`/`Update` dispatch only | ❌ | No sequence boxes, gap buffer, difference recovery, or channel state. |
| `telegram/dcs` (resolvers: Plain / MTProxy / Websocket / DNS; DC lists) | — | ❌ | |
| DC migration / redirect / auth transfer (`invoke.go`, `migrate_to_dc.go`, `dc_transfer.go`) | — | ⛔ | Deferred by `openspec/specs/multi-dc` (spec written, implementation unstarted). |
| `pool` + CDN pool manager | — | ❌ | No connection pooling. |
| `session` backends (File / Memory / JS / Telethon / TDesktop) | client `SessionData` blob only | 🟡 | No storage-backend trait, no `td`/tdesktop/Telethon-compatible readers. |
| `middleware.go` + `flood_wait.go` (rate limit, retry, tracing decorators) | `FLOOD_WAIT` *parsing* only | 🟡 | No `Invoker` decorators or automatic retry. |
| keepalive / ping / reconnect / background receive | server answers ping; client none | ❌ | Updates are read only while a call awaits its reply. |
| `uploader` (+ `source`) | — | ❌ | TL types only. |
| `downloader` (+ CDN, parallel, verify, web) | — | ❌ | TL types only. |
| `message` builders (+ `entity`/`html`/`markdown`/`rich`/`styling`/`markup`/`inline`/`unpack`) | — | ❌ | |
| `query` pagination (+ `cached`, `hasher`) | — | ❌ | |
| `peers` (+ `members`) | — | ❌ | |
| `thumbnail` (incl. SVG) | — | ❌ | |
| `takeout` | — | ❌ | |
| `deeplink` | — | ❌ | |
| `tljson`, `tdjson` (TDLib-style `@type` JSON) | — | ❌ | |
| `tdp` pretty-printer | — | ❌ | |
| `fileid` (Bot-API/TDLib file-id codec) | — | ❌ | |
| `calls` VoIP (1:1 + group, pion/WebRTC) | — | ❌ | |
| `tg/e2e` secret chats | — | ❌ | TL types only (no `tg/e2e` schema crate). |
| `InvokeJSON` / TDLib JSON invoke | — | ❌ | |
| `oteltg` OpenTelemetry | — | ❌ | No tracing/metrics. |
| device / TDesktop mimicry (`device.go`) | — | ❌ | |

## 4. Server runtime & multi-DC (rtd's own goal, beyond `td`)

`td` ships only an in-process test server (`tgtest`). `rtd`'s product is its
server runtime, so these are gaps against `rtd`'s own specs, not against `td`.

| Area | Status | Notes |
|---|---|---|
| Multi-DC routing (`multi-dc` spec) | ⛔ | "Not started"; deferred by frozen decision. Includes multi-DC advertisement, per-DC keys, DC-aware sessions. |
| DC migration / redirect directives / authorization transfer | ⛔ | Unstarted (`multi-dc` spec). |
| Durable cross-DC delivery (outbox: claim/project/ACK) | ⛔ | Unstarted. |
| Storage backends (PostgreSQL + Redis) | ⛔ | Only in-memory `LocalCore` / remote gRPC; storage traits reserved. |
| Server-side updates / pts-qts-seq delivery | ❌ | No update cursor or delivery logic; only the `UpdateState` protobuf message exists in `rtd-edge-proto`. |
| Per-method rate limiting / `FLOOD_WAIT` emission | 🟡 | Connection/handshake/frame-byte admission budgets only. |
| Production exchange parameters | 🟡 | Server uses fixed `TestServerRng`. |

## 5. Testing & tooling

| `td` area | `rtd` | Status | Notes |
|---|---|---|---|
| `tg` codegen (`gen`, `tlprofile`) | `rtd-codegen` + `rtd-tlprofile` | ✅ | Layers 225–232 vs `td`'s 225–231. |
| `tg/e2e` generated | — | ❌ | |
| `tgtrace` generated | — | ❌ | |
| `tgtest` (+ `cluster`, `services`) | `rtd-server/tests/e2e.rs` + fake core | 🟡 | Single-DC, in-process only. |
| `tgmock` | closures/mocks | 🟡 | |
| `_fuzz` corpora (`flow`, `handle_message`, `rsa`) | — | ❌ | |
| `examples/*` | `rtd-edge` examples | 🟡 | |

---

## Not gaps (already done, despite appearances)

These were candidate gaps in earlier notes but are implemented:

- **Edge core-exec bearer token** — done, archived change
  `2026-11-02-add-edge-coreexec-token` (`--core-token` /
  `RTD_EDGE_CORE_TOKEN`, applied as `authorization: Bearer …` on every call).
- **Edge container artifact** — `Dockerfile` builds `rtd-edge` and runs it
  non-root.
- **gRPC Edge↔Core contract** — `rtd-edge-proto` pins
  `telesrv.coreexec.v1.CoreExecService` (v5) with `add-edge-coreexec-binding`.
- **Server-side `get_future_salts`** — answered by `rtd-server::actor` (basic;
  reuses the current salt).

## Intentional scope decisions

From `openspec/config.yaml` and `NEXT_SESSION.md`:

- Primary scope is the **server-facing core** and the **multi-DC server
  runtime**. The client library is secondary; its convenience helpers
  (`uploader`, `downloader`, `message`, `query`, `peers`, `takeout`, `calls`,
  `fileid`, …) are **not** required for the server goal and are not planned yet.
- **Multi-DC must not be started** until explicitly unblocked (frozen decision);
  the `multi-dc` spec is written but unimplemented.
- `unsafe` is forbidden workspace-wide; clippy `all = deny`; **no `allow(...)`
  anywhere**, including generated code and tests.

## Constraints that affect any gap work

- **Codegen size budget.** `crates/rtd-tlprofile/src/lib.rs` is ~395,109 lines
  (~98.8% of the declared 400k-line budget). Any change that emits much
  generated code (or adds a layer) may breach it; raise the budget in
  `crates/rtd-codegen/src/output.rs` with a decision record, or reduce per-lane
  emission first.
- **Windows Application Control** blocks freshly built executables and
  proc-macro DLLs (`os error 4551`), so `cargo test`/`run`/`fmt` must run under
  WSL; `build`/`check`/`clippy` work on Windows.
- **Code generation is committed and never hand-edited**; regenerate with
  `cargo run -p rtd-codegen -- generate` and keep the freshness gate green.

## Suggested sequencing for closing gaps

1. **Client conformance core** (highest leverage, contained): background receive
   loop → multi-salt `get_future_salts` → keepalive/reconnect → MTProxy dialer
   abstraction (compose FakeTLS + obfuscated2). Each is its own OpenSpec change.
2. **2FA / SRP**: add the server-side SRP verifier in `rtd-crypto`, then client
   `account.getPassword` → `InputCheckPasswordSRP`.
3. **Updates gap recovery** (pts/qts/seq manager) to make the client's update
   handling real.
4. **Client convenience layer** if the client becomes a first-class goal
   (uploader/downloader first, then message/query/peers).
5. **Multi-DC server program** — only once explicitly unblocked; plan the
   codegen budget first.
