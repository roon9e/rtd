# `td` → `rtd` parity

A working comparison of the Go reference [`td`](https://github.com/iamxvbaba/td) against the
Rust `rtd` workspace. Protocol/core parity is essentially complete; the remaining gaps are
client-library ergonomics.

Legend: **done** — implemented; **partial** — an equivalent exists but is missing surface;
**missing** — no equivalent; **n/a** — Go/tooling infrastructure the Rust library does not
need to mirror.

## Crate/package mapping

| `td` package | `rtd` equivalent | Status |
|---|---|---|
| `bin` | `rtd-bin` | done |
| `crypto`, `crypto/srp` | `rtd-crypto` | done |
| `exchange` | `rtd-exchange` | done |
| `proto`, `proto/codec` | `rtd-proto` | done |
| `mtproto`, `mtproto/salts` | `rtd-proto` + `rtd-client::session` | done |
| `mtproxy/*`, `transport` | `rtd-transport` | done |
| `wsutil` | `rtd-client::ws`, `rtd-client::transport` | done |
| `mt` | `rtd-mt` | done |
| `tg` | `rtd-tg` | done |
| `tgerr` | `rtd-error` | done |
| `tmap` | `rtd-tmap` | done |
| `tlprofile` | `rtd-tlprofile` | done |
| `tgtrace` | `rtd-tgtrace` | done |
| `rpc` (engine) | `rtd-client::client` | done |
| `clock` | `rtd-util::clock` | done |
| `clock/ntp` | — | missing |
| `syncio`, `tdsync` | — | n/a (Rust ownership/tokio) |
| `ascii` | — | missing (trivial) |
| `constant` | — | missing |
| `tdjson` | `rtd-client::json` | done |
| `tdp` | `rtd-client::json::format` (name#id, not a field dump) | partial |
| `fileid` | `rtd-client::fileid` | done |
| `pool` | `rtd-client::pool` | done |
| `session`, `session/tdesktop` | `rtd-client::session`, `storage`, `tdesktop` | done |
| `gen`, `_tools`, `_schema` | `rtd-codegen`, `schema/` | done |
| `oteltg` | `rtd-client::middleware` (abstract `Tracer` only) | partial |
| `keyparser` | — | n/a (key-refresh tooling) |
| `testutil`, `tgmock`, `tgtest`, `tgacc` | `interop/`, `rtd-tg-e2e`, test modules | n/a |
| `examples`, `cmd` | — | n/a |
| `telegram` (client root) | `rtd-client::client`, `async_client`, `manager` | done/partial |
| `telegram/auth` | `rtd-client::auth`, `client`, `qr` | partial (`Flow` missing) |
| `telegram/auth/srpguard` | — | missing |
| `telegram/calls` | `rtd-client::calls`, `media` | done |
| `telegram/cdn` | `rtd-client::cdn` | done |
| `telegram/dcs` | `rtd-client::dcs` | done |
| `telegram/deeplink` | `rtd-client::deeplink` | done |
| `telegram/downloader` | `rtd-client::downloader` | done |
| `telegram/uploader[+source]` | `rtd-client::uploader` | done |
| `telegram/takeout` | `rtd-client::takeout` | done |
| `telegram/thumbnail` | `rtd-client::thumbnail` | done |
| `telegram/peers[+members]` | `rtd-client::peers`, `members` | done |
| `telegram/updates[+hook]` | `rtd-client::updates` | done |
| `telegram/query/*` | `rtd-client::pagination` | partial |
| `telegram/tljson` | `rtd-client::json` | done |
| `telegram/message` | `rtd-client::message` | partial |
| `telegram/message/entity` | `rtd-client::entity` | done |
| `telegram/message/styling` | `rtd-client::entity` | done |
| `telegram/message/rich` | — | missing |
| `telegram/message/markup` | `rtd-client::markup` | done |
| `telegram/message/inline` | `rtd-client::inline` | done |
| `telegram/message/peer` | `rtd-client::peers` | done |
| `telegram/message/unpack` | `rtd-client::unpack` | done |

## Remaining gaps

1. **High-level authorization flow** (`telegram/auth`): no `Flow`/`UserAuthenticator` state
   machine, no `Status`/sign-up/TOS, no password reset/recovery. *Effort: M, no new deps.*
2. **`srpguard`**: locked/zeroizing buffer for the 2FA password. *Effort: S, optional dep.*
3. **Rich messages** (`telegram/message/rich`): `PageBlock`/`RichText` builders and
   HTML/Markdown parsing; `message.rs` currently sets `rich_message: None`. *Effort: L, no new deps.*
4. **Styling / entity builder** (`telegram/message/styling`, `entity`): programmatic styled-text
   composition into entities. *Effort: M, no new deps.* — **done**, see `rtd-client::entity`
   (`EntityBuilder`/`EntityKind`; parse and build).
5. **OpenTelemetry** (`oteltg`): only an abstract `Tracer` trait exists; no OTel integration.
   *Effort: S–M, new deps.*
6. **Constants** (`constant`): TDLib peer-id encoding and well-known IDs/limits. *Effort: S.*
   — **done**, see `rtd-client::constant`.
7. **TL pretty-printing** (`tdp`): a reflection-style field dump of any object. *Effort: M.*
8. **NTP clock** (`clock/ntp`): an NTP-calibrated clock. *Effort: M, new dep.*
9. **ASCII predicates** (`ascii`). *Effort: S.*
10. **Staged message senders** (`telegram/message`): audio/video/gif/dice/game/venue/webpage
    helpers. *Effort: M, no new deps.* — **done**, see `rtd-client::message`
    (`send_audio`/`send_video`/`send_gif`/`send_dice`/`send_venue`/`send_game`/`send_webpage`).
    Sticker/typing/draft/report/join and schedule options remain.
11. **Query/pagination extras**: featured-stickers iterator and `query/cached` helpers.
    *Effort: S–M, no new deps.*
12. **Markup richness** (`telegram/message/markup`): reply keyboards, force-reply, richer
    buttons. *Effort: S, no new deps.* — **done**, see `rtd-client::markup`
    (`reply_keyboard`/`reply_keyboard_hide`/`reply_keyboard_force_reply`).
13. **Client ergonomics** (`telegram` root): `Self`, environment-based construction,
    `InvokeRawJSON`. *Effort: S–M.* This also needs **bare-vector RPC results** (e.g.
    `users.getUsers` returns `Vector<User>`), which `Client::invoke` cannot currently decode.
14. **Bare-vector results**: `Client::invoke` decodes only `Object`; methods that return a bare
    `Vector<T>` are unusable. *Effort: M, architectural.* — **done**, see `Client::invoke_raw`
    / `InvokeResult`.

## Out of scope

- Server storage (PostgreSQL/Redis) and the full Rust server/edge are frozen/out of scope.
- Go concurrency glue (`syncio`, `tdsync`), CLI tools (`cmd/*`), examples, and key-refresh
  tooling (`keyparser`) are not client-library surface.
- Test infrastructure (`testutil`, `tgmock`, `tgtest`, `tgacc`) is replaced by `interop/`,
  `rtd-tg-e2e`, and the external-peer gate.
