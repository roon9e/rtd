# Proposal

## Why

Three small but widely used helpers are missing: Bot-API/TDLib **file ids** (`fileid`),
**thumbnail reconstruction** (`thumbnail`), and **deep-link parsing** (`deeplink`). `td` ships
all three as top-level packages.

## What Changes

- `fileid`: encode/decode Bot-API and TDLib file ids (`FileID`, `Type`, RLE,
  `FromDocument`/`FromPhoto`, `AsInputFileLocation`/`AsInputWebFileLocation`).
- `thumbnail`: reconstruct a JPG payload from stripped thumbnails (`Expand`) and handle SVG.
- `deeplink`: parse `tg:`/`t.me`/`telegram.me` deep links into `Resolve`/`Join`/`BusinessChat`.

## Capabilities

### New Capabilities

- `client-media-helpers`: file-id, thumbnail and deep-link helpers.

## Impact

- **Crates**: `rtd-client` (or a small shared `rtd-media` module); uses existing `rtd-bin`.
- **Non-goals**: rendering thumbnails or SVG.
