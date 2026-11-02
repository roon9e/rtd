# Proposal

## Why

`rtd` is a server-facing MTProto 2.0 core. The workspace already contains every wire-level
client primitive — the requester auth-key exchange (`rtd-exchange::client_exchange`), TCP
framing (`rtd-proto::codec`), obfuscation (`rtd-transport::{Obfuscated2, FakeTls}`), MTProto
2.0 encryption (`rtd-crypto`) and a complete canonical-layer-232 TL codec (`rtd-tg::{Method,
Object}`) — but nothing composes them into a client. There is no connection driver, no
client-side session, and no request/response correlation at the canonical layer.

## What Changes

- A new `rtd-client` crate: a synchronous MTProto 2.0 client core that
  - connects over TCP (optional obfuscated2/FakeTLS) using `rtd-proto`'s codec,
  - completes the auth-key exchange with `rtd-exchange::client_exchange`,
  - runs an encrypted session (message ids, seqno, salt, MTProto 2.0 encryption),
  - correlates RPC results to requests and returns canonical `rtd_tg::Object` values,
  - establishes the first session with `invokeWithLayer`/`initConnection` at layer 232.
- The core is transport-agnostic behind a small `Transport` trait, so it is exercised by an
  in-memory duplex test and by the official engine's `mtproto-testserver` peer (env-gated),
  exactly like `rtd-exchange`'s gates.

## Capabilities

### New Capabilities

- `client-runtime`: an MTProto 2.0 client that establishes an encrypted session with a
  server and invokes canonical-layer methods.

## Impact

- **Crates**: adds `rtd-client` (depends on `rtd-bin`, `rtd-crypto`, `rtd-error`,
  `rtd-exchange`, `rtd-mt`, `rtd-proto`, `rtd-tg`, `rtd-transport`, `rtd-util`). No existing
  crate changes behavior.
- **Non-goals (this change)**: authorization (`auth.sendCode`/sign-in), 2FA, the updates
  engine, DC migration/exports, persistent session storage, and an async/tokio driver. The
  core is synchronous and sans-IO behind `Transport`; an async driver can wrap it later.
- **Compatibility**: speaks MTProto 2.0 at canonical layer 232; interoperates with real
  Telegram servers and the official test server by construction.
