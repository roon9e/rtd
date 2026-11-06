# Design

## Decision record: canonical layer 232 → 233

Canonical-layer bumps are accepted only as their own change with a checked-in decision record and
green cross-implementation tests. This is that record.

## Source

- Repository: `https://github.com/telegramdesktop/tdesktop.git`, source
  `Telegram/SourceFiles/mtproto/scheme/api.tl`, stable/`ready` commit
  `a6df181ac41763a9ff9edae264dad407fb1c35ec` (git blob
  `4e29f2deb45a083196a8c7ac09cdedf2600c1a10`, normalized SHA-256
  `c2bbefcaec3285ba46c8af8f429c7012fe5aea253fd16d4be089f91c7c3fa71e`).
- `rtd`'s `schema/layers/layer-233.tl` and each layer file's commit/blob/SHA-256 match the `td`
  reference byte-for-byte, as does the exported `policy.json`.

## Schema delta (relative to 232)

27 constructors added, three new function groups, two changed wire-visible definitions:

- `user` (unchanged id `#b1b8cc83`) gains `bot_forum_sidebar:flags2.22?true`.
- `channel` (unchanged id `#7ad15240`) gains `forum_sidebar:flags2.23?true`.
- `channels.toggleForum` moves from `#3ff75734` to `#a2952b54` with a **required** `sidebar:Bool`.
- New: `inputMediaWidget`/`inputMediaWidgetId`/`messageMediaWidget`, the bot-inline and
  widget-query types, `userWallet`, `messages.*Widget*`, `messages.transcribeAudioFile`,
  `messages.getTranscriptionLangs`, and six new `Update` constructors.

The MTProto service schema (`mt.tl`) is unchanged.

## Compatibility decisions (reviewed in `LAYER233_REVIEW.md`, adopted verbatim)

- `channels.toggleForum` in 225–232 → **reject admission** (a historical request cannot express
  the required `sidebar` decision; defaulting it could silently overwrite the account's layout).
- Canonical `channels.toggleForum` encoded for 225–232 → **drop** `sidebar`, emit `#3ff75734`.
- `channel.forum_sidebar`, `user.bot_forum_sidebar` for 225–232 → **drop** (layout metadata only).
- Six new updates for 225–232 → **drop** (unavailable to old profiles).
- New widget methods/results and `WidgetQueryType`/`UserWallet` types → **canonical-only**.
- `payments.getStarGiftWithdrawalUrlLegacy#d06e93a8` remains canonical-only; exact profile 232
  defines `#c8ca817b`.

Policy: 305 entries (249 retained unchanged + 56 new obligations, none using a hook).

## Budget

The generated sidecar grows with the ninth profile to 13,798,265 bytes / 418,281 lines, beyond the
previous 400k-line ceiling but within 16 MiB. The line ceiling is raised to 500k; bytes are
unchanged. This mirrors `td` adding a ninth profile while keeping the byte budget.

## Verification

- Schema/policy parity with `td`: all layer files 225–233 and `policy.json` hash-equal; manifest
  commit/blob/SHA-256 per layer equal; canonical 233 in both.
- `rtd-codegen verify` and `generate` report `canonical=233 enabled=[225..233]` and accept the
  policy with no stale or unresolved obligation.
- `cargo fmt`, `clippy -D warnings`, `workspace test --locked`, and the generation-freshness check
  are green.

## Risks / Trade-offs

- [Version skew] A gramsrv core pinned to `td v1.7.0` (229) is unaffected; the edge keeps its
  configured layer. Only the client default rises to 233.
