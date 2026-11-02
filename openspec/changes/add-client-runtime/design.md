# Design

## Context

The workspace's protocol crates are sans-IO and symmetric enough that only composition is
missing. `rtd-exchange` already implements the requester half of the auth-key exchange
(`client_exchange`) over a synchronous `Stream`; `rtd-transport` already exposes
`Obfuscated2::handshake`/`FakeTls::client_handshake`; `rtd-proto::codec` frames TCP; and
`rtd-tg` provides `Method`/`Object` for canonical layer 232. `rtd-server` shows the server
composition of the same pieces.

## Goals / Non-Goals

**Goals**

- A client that goes from a bare `TcpStream` to a successful canonical-layer method call
  (`help.getConfig` via `invokeWithLayer`/`initConnection`), returning `rtd_tg::Object`.
- Reuse of the existing exchange, transport, crypto and codec crates with no wire changes.

**Non-Goals**

- Authorization, 2FA, updates, DC migration, persistence, and an async runtime driver.

## Decisions

### D1. A synchronous core behind a `Transport` trait

The client core is synchronous and sans-IO behind a small `Transport` trait
(`send_frame`/`recv_frame`), so it composes directly with the synchronous
`rtd_exchange::client_exchange`/`Stream` and PT. The production transport is TCP + `rtd-proto`
codec; an in-memory duplex implements the same trait for deterministic tests. An async/tokio
driver (bridging the socket with channels and `spawn_blocking`, as `rtd-server` does) is a
follow-up change; it does not change this core.

### D2. Reuse `rtd_proto::Session` for the client side

`rtd_proto::Session` already allocates message ids and seqnos and tracks the salt; it is
symmetric for allocation even though its `accept_inbound` is server-biased. The client uses
`Session::with_id`, `allocate`, `salt`, `set_salt` (as `rtd-exchange`'s `peer_gate` does) and
does its own inbound validation (session id, replay window, content/server-response types)
rather than `accept_inbound`.

### D3. RPC correlation, acks and salt refresh

`Client::invoke` allocates a content message, encrypts it, and waits for the matching
`rpc_result` keyed on the request's message id, unwrapping `msg_container` and `gzip_packed`
and dispatching through `rtd_tg::Object`/`rtd_mt::RpcResult`. Server messages with an odd
seqno are acknowledged with `msgs_ack`; a `bad_server_salt` notification updates the salt and
resends. A per-request retry (bounded) resends an unacknowledged request.

### D4. The first call establishes the session

The first `invoke` wraps the request in `invokeWithLayer { layer: 232, query:
initConnection { api_id, device_model, system_version, app_version, system_lang_code,
lang_pack, lang_code, query } }`, mirroring `td`'s `manager.Conn.Init`, and returns the inner
result (typically `help.getConfig`). Subsequent calls are sent directly.

### D5. Transport selection

The production transport wraps a `TcpStream` with a `rtd-proto` codec (`Intermediate` by
default, matching the test peer) and optionally `rtd_transport::Obfuscated2`/`FakeTls` with a
proxy secret. Codec/obfuscation are constructor parameters, not auto-detected.

## Risks / Trade-offs

- [Blocking I/O limits reuse in async services] → the core is behind `Transport`; an async
  driver follows, and the server already owns the async path.
- [No production RSA keys bundled] → the caller supplies `RsaPublicKey`s, as
  `rtd-exchange` already requires; the test peer advertises its own.
- [Partial message handling could diverge from `td`] → unsupported service messages are
  reported/skipped, and the peer gate exercises a real server.
