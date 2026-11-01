# Proposal

## Why

rtd's protocol core is finished and proven: it serializes TL, frames MTProto 2.0
messages, runs the auth-key exchange, and already interoperates with the official
engine's test server at the handshake level. But there is no *server*. `rtd-server`
and the `rtd` facade are empty skeletons, `rtd-edge` only checks a contract version,
and the encrypted session layer, RPC dispatch, and edge boundary do not exist yet.

Stage 3 of the foundation's migration plan ("server edge + RPC + sidecar boundary") is
what turns the primitives into a runnable, multi-layer MTProto edge that Go `gramsrv`
can adopt as a sidecar without rewriting its core. Everything the edge needs is already
specified behavior in `server-edge`, `server-rpc`, and `edge-service`; this change
implements it and adds the pieces those specs left open: the encrypted message envelope,
the extra ingress transports, the state-ownership split, and the shape of the gRPC
boundary.

## What Changes

- **Aligning already-committed crates** so the edge is a true drop-in replacement
  (audited against gramsrv's `origin/v2` edge dependencies): reshape `rtd-edge-proto` to
  mirror `CoreExecService`, extend the generated `rtd-tlprofile` runtime with the
  wrapper/call-identity/result surface the edge needs, add MTProxy secret parsing to
  `rtd-transport`, and add a combined wire-id name registry (the `td/tmap` equivalent).
- **Encrypted MTProto 2.0 session framing**: the `auth_key_id`-keyed envelope with a
  message key and AES-IGE payload, server-salt derivation and update, and session
  identity. (Today only the plaintext exchange messages exist.)
- **`rtd-server` edge runtime** on tokio:
  - inbound transport detection and unwrapping for TCP (abridged / intermediate /
    padded-intermediate / full), obfuscated2, WebSocket, and FakeTLS/MTProxy server mode;
  - the server side of the key exchange wired to inbound connections;
  - a per-connection **actor** with a single writer assigning message ids and sequence
    numbers, no locks on the hot path;
  - message-id/sequence validation, replay protection, and duplicate-tolerance;
  - session lifecycle (`new_session_created` once, resource release on disconnect);
  - service messages: `ping`/`pong`/`ping_delay_disconnect`, `msgs_ack`, future-salt,
    `resend`, bad-message/bad-salt, `destroy_session`, ack requests;
  - admission budgets (total connections, per source, concurrent handshakes, aggregate
    frame bytes);
  - per-profile result and update encoding for the session's negotiated layer.
- **RPC layer**: invocation-wrapper unwrapping with a depth bound, layer binding
  (explicit selection or inherited from the auth key, corrected in client message-id
  order, fail-closed for unsupported layers), result-identity binding, TL error mapping,
  unknown-method fallback, handler isolation, a **pluggable router trait**, and a small
  set of **built-in methods** (`help.getConfig`, `help.getNearestDc`, `ping`) to prove
  the path.
- **State ownership (mirrors gramsrv v2)**: the core durably owns auth keys and
  session-layer evidence; the edge keeps only transient per-connection state and an
  auth-key cache, saving a new key to the core before confirming and resolving keys it
  does not hold from the core.
- **Edge boundary** (mirrors gramsrv v2's `CoreExecService`): an in-process `Core` trait
  the edge dispatches through, plus a **tonic gRPC adapter** in which the edge dials the
  core's `DispatchAdmitted`; multi-layer admission runs in the edge, and durable auth keys
  and layer evidence live in the core. `rtd-edge` becomes a runnable service.
- **Verification**: sans-IO session state-machine tests driven by injected time and
  randomness, an in-process end-to-end test (test client ↔ rtd edge ↔ fake core) across
  every ingress transport, and an extension of the official-engine **external-peer gate**
  to a full encrypted session (ping/pong), not just the handshake.

## Capabilities

### New Capabilities

None. Durable storage is owned by the core and reached over the boundary (mirroring
gramsrv v2), so this change only modifies existing capabilities.

### Modified Capabilities

- `protocol-crypto`: adds the encrypted MTProto 2.0 message envelope (auth-key-id,
  message key, AES-IGE payload and inner header) and server-salt derivation.
- `server-edge`: adds the encrypted session layer, WebSocket and FakeTLS/MTProxy ingress,
  the storage-backed session/key lifecycle, and the runtime budgets that serve them.
- `server-rpc`: adds the pluggable request router and the built-in service methods.
- `edge-service`: mirrors gramsrv v2's `CoreExecService` — the edge calls the core, the
  core owns durable auth keys and layer evidence, admission runs in the edge, and an
  in-process `Core` implementation shares the boundary with the tonic adapter.
- `interop-testing`: extends the external-peer gate to a complete encrypted session and
  adds the in-process end-to-end test.

## Impact

- **Crates**: new modules in `rtd-crypto` (encrypted message envelope, server salt), and
  `rtd-proto` (sans-IO session state), plus `rtd-server` (transport ingress, session actor,
  service messages, rpc, core-as-trait); `rtd-edge-proto` is reshaped to mirror gramsrv v2's
  `CoreExecService`; `rtd-edge` becomes a service binary; the `rtd` facade re-exports the
  runtime surface. Generated crates are untouched (canonical layer 232).
- **Dependencies**: adds `tokio`, `tonic`, `tokio-tungstenite`/`tungstenite`, and a
  hashing/HTTP helper where needed; keeps `prost`. No GPL dependency; the official engine
  stays a run-only test peer.
- **Compatibility**: the gRPC contract stays byte-oriented and versioned, so the core can
  be upgraded independently. No change to Go `gramsrv`; adoption is opt-in per DC.
- **Security**: keys and salts flow through the existing secure-random source and stay
  redacted in logs; admission bounds remain enforced on the encrypted path.
