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
| `clock/ntp` | `rtd-util::ntp` (feature `ntp`) | done |
| `syncio`, `tdsync` | — | n/a (Rust ownership/tokio) |
| `ascii` | `rtd-client::ascii` | done |
| `constant` | `rtd-client::constant` | done |
| `tdjson` | `rtd-client::json` | done |
| `tdp` | `rtd-client::tdp` | done |
| `fileid` | `rtd-client::fileid` | done |
| `pool` | `rtd-client::pool` | done |
| `session`, `session/tdesktop` | `rtd-client::session`, `storage`, `tdesktop` | done |
| `gen`, `_tools`, `_schema` | `rtd-codegen`, `schema/` | done |
| `oteltg` | `rtd-client::middleware` (`Tracer`) + `OtelTracer` (feature `otel`) | done |
| `keyparser` | — | n/a (key-refresh tooling) |
| `testutil`, `tgmock`, `tgtest`, `tgacc` | `interop/`, `rtd-tg-e2e`, test modules | n/a |
| `examples`, `cmd` | — | n/a |
| `telegram` (client root) | `rtd-client::client`, `async_client`, `manager` | done |
| `telegram/auth` | `rtd-client::auth`, `client`, `qr` | done |
| `telegram/auth/srpguard` | `rtd-client::auth::HeldPassword` | done |
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
| `telegram/query/*` | `rtd-client::pagination` | done |
| `telegram/tljson` | `rtd-client::json` | done |
| `telegram/message` | `rtd-client::message` | done |
| `telegram/message/entity` | `rtd-client::entity` | done |
| `telegram/message/styling` | `rtd-client::entity` | done |
| `telegram/message/rich` | `rtd-client::rich` | done |
| `telegram/message/markup` | `rtd-client::markup` | done |
| `telegram/message/inline` | `rtd-client::inline` | done |
| `telegram/message/peer` | `rtd-client::peers` | done |
| `telegram/message/unpack` | `rtd-client::unpack` | done |

## Coverage estimate

Two lenses. **Package-level** counts the mapping above: of 52 data rows, 4 are `n/a` (Go
test/CLI/tooling), leaving **48/48 in-scope packages mapped to a Rust equivalent**. **Surface
weighted** is an estimate that weights each area by how much client-facing API it carries.

| Area | Coverage | Note |
|---|---|---|
| Protocol/core (`bin`, `crypto`, `exchange`, `proto`, `mtproto`, `mtproxy`, `transport`, `mt`, `tg`, `tgerr`, `tmap`, `tlprofile`, `tgtrace`) | **~100%** | mirrored crate-for-crate |
| Client runtime & transport (`telegram` root, `rpc`, `pool`, `session`, `session/tdesktop`, `dcs`, `cdn`, `wsutil`, `mtproxy` dialer) | **~95%** | env-based construction omitted |
| Auth (`auth`, `qrlogin`, `srpguard`) | **~95%** | `Flow`, sign-up/TOS, 2FA, QR, held password |
| Messaging (`message`, `entity`, `styling`, `rich`, `markup`, `inline`, `peer`, `unpack`) | **~92%** | rich/structured coverage is a broad subset |
| Media/IO (`uploader`, `downloader`, `fileid`, `thumbnail`, `takeout`, `deeplink`) | **~100%** | |
| Queries & updates (`query/*`, `updates`, `peers`) | **~95%** | main iterators + search + cache |
| Interop/debug (`tdjson`, `tljson`, `tdp`, `oteltg`, `clock/ntp`) | **~95%** | `tdp` field-fidelity limit below |

**Overall estimate: ~93–97% of the `td` client library** by weighted surface, and ~100% of the
packages that correspond to a client-library concern. Uncertainty is roughly ±3 percentage points;
the lower bound reflects the one accepted `tdp` divergence, the upper bound treats "package mapped
+ primary use cases covered" as parity.

## Gap closure log (all done)

1. **High-level authorization flow** (`telegram/auth`): no `Flow`/`UserAuthenticator` state
   machine, no `Status`/sign-up/TOS, no password reset/recovery. *Effort: M, no new deps.* —
   **done**, see `rtd-client::auth::{Flow, Status}` plus `default_code_settings`; the low-level
   recovery calls (`request_password_recovery`/`check_recovery_password`/`recover_password`)
   were already present.
2. **`srpguard`**: locked/zeroizing buffer for the 2FA password. *Effort: S, optional dep.* —
   **done**, see `rtd-client::auth::HeldPassword` (zeroized on drop).
3. **Rich messages** (`telegram/message/rich`): `PageBlock`/`RichText` builders and
   HTML/Markdown; `message.rs` currently sets `rich_message: None`. *Effort: L, no new deps.* —
   **done**, see `rtd-client::rich` (`RichMessage`, `text`/`bold`/`italic`/`fixed`/`link`/`concat`,
   paragraphs/headings) plumbed through `RequestBuilder::rich_message` and `edit_rich_message`.
   Raw HTML/Markdown is parsed server-side (`inputRichMessageHTML`/`Markdown`).
4. **Styling / entity builder** (`telegram/message/styling`, `entity`): programmatic styled-text
   composition into entities. *Effort: M, no new deps.* — **done**, see `rtd-client::entity`
   (`EntityBuilder`/`EntityKind`; parse and build).
5. **OpenTelemetry** (`oteltg`): only an abstract `Tracer` trait exists; no OTel integration.
   *Effort: S–M, new deps.* — **done**, see `rtd-client::OtelTracer` behind the optional `otel`
   feature (uses the OpenTelemetry global tracer provider).
6. **Constants** (`constant`): TDLib peer-id encoding and well-known IDs/limits. *Effort: S.*
   — **done**, see `rtd-client::constant`.
7. **TL pretty-printing** (`tdp`): a reflection-style field dump of any object. *Effort: M.* —
   **done**, see `rtd-client::tdp` (`format`/`compact`/`label`), using the generated `Debug`
   representation. Byte fields render as decimal lists (base64 remains available via
   `json::encode_object`).
8. **NTP clock** (`clock/ntp`): an NTP-calibrated clock. *Effort: M, new dep.* — **done**, see
   `rtd-util::ntp::NtpClock` behind the optional `ntp` feature (hand-rolled SNTP, no dependency),
   re-exported from `rtd-client` under `ntp`.
9. **ASCII predicates** (`ascii`). *Effort: S.* — **done**, see `rtd-client::ascii`.
10. **Staged message senders** (`telegram/message`): audio/video/gif/dice/game/venue/webpage
    helpers. *Effort: M, no new deps.* — **done**, see `rtd-client::message`
    (`send_audio`/`send_video`/`send_gif`/`send_dice`/`send_venue`/`send_game`/`send_webpage`).
    Sticker/typing/draft/report/join and schedule/reply options are also done
    (`send_sticker`/`set_typing`/`save_draft`/`report`/`join_channel`,
    `RequestBuilder::reply_to`/`schedule_date`).
11. **Query/pagination extras**: featured-stickers iterator and `query/cached` helpers.
    *Effort: S–M, no new deps.* — **done**, see `fetch_featured_stickers` and
    `QueryCache`/`cached`.
12. **Markup richness** (`telegram/message/markup`): reply keyboards, force-reply, richer
    buttons. *Effort: S, no new deps.* — **done**, see `rtd-client::markup`
    (`reply_keyboard`/`reply_keyboard_hide`/`reply_keyboard_force_reply`).
13. **Client ergonomics** (`telegram` root): `Self`, `InvokeRawJSON`. *Effort: S–M.* — **done**,
    see `Client::self_user`, `parse_json` and `Client::invoke_raw_json` (on top of bare-vector
    results). Environment-based construction remains out of scope.
14. **Bare-vector results**: `Client::invoke` decodes only `Object`; methods that return a bare
    `Vector<T>` are unusable. *Effort: M, architectural.* — **done**, see `Client::invoke_raw`
    / `InvokeResult`.

## Out of scope

- Server storage (PostgreSQL/Redis) and the full Rust server/edge are frozen/out of scope.
- Go concurrency glue (`syncio`, `tdsync`), CLI tools (`cmd/*`), examples, and key-refresh
  tooling (`keyparser`) are not client-library surface.
- Test infrastructure (`testutil`, `tgmock`, `tgtest`, `tgacc`) is replaced by `interop/`,
  `rtd-tg-e2e`, and the external-peer gate.

## Residual differences (fidelity, not package coverage)

These account for most of the estimate's lower bound. Entries marked **closed** were narrowed by
`add-client-parity-fidelity`.

- **`tdp`** (accepted divergence): the field dump uses the generated `Debug` form — byte fields
  render as decimal lists and `date` fields as integers. The alternative (generated per-field
  metadata and accessors) is deliberately **omitted**: it would drastically increase compile time,
  enlarge the binary, and complicate the generator, for a debug-only convenience. Base64 bodies
  remain available through `json::format`/`encode_object`.
- **Rich messages**: paragraph/heading plus list, preformatted, quote, divider, details and
  header/subheader/subtitle/footer blocks; not every `PageBlock`/`RichText` constructor is wrapped.
- **Query/pagination** *(closed)*: history, dialogs, participants, photos, blocked, featured
  stickers, message search and a cached helper.
- **Markup** *(closed)*: inline (callback/URL/web-view/copy/switch-inline/user-profile/buy/game/
  disabled) and reply keyboards, bot-menu buttons and `requestPeer`.
- **Message senders** *(closed)*: audio/video/gif/dice/venue/game/webpage/sticker/poll/geo/contact,
  story, paid media, invoice and to-do, plus reply/schedule/silent options.
- **`oteltg`** *(closed)*: spans and a per-call counter; no histograms.
- **Client construction** *(closed)*: environment credential helpers (`api_credentials_from_env`).

## Verification basis

- Package mapping and fidelity notes: this document.
- Wire compatibility: the committed cross-check vectors (`interop/vectors.json`), the codegen
  freshness gate, and the external-peer gate against the official Rust engine.
- Local gate (`fmt`, `clippy -D warnings`, `build`/`test --locked`, generation freshness); the
  `otel`/`ntp` features are checked by CI's `optional features` job.
