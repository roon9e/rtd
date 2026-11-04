# Proposal

## Why

`td`'s `telegram/message/entity` *builds* styled text and its entities (`entity.Builder`,
`Format`, `Plain`, generated `Bold`/`Italic`/... formatters). `rtd-client::entity` only
*parses* HTML/Markdown, so callers that construct text in code cannot attach entities and must
compute UTF-16 offsets by hand.

## What Changes

- Add `EntityBuilder` with `plain`, `styled` and `styled_with` span methods that track UTF-16
  offsets.
- Add `EntityKind` covering the common entities and a `utf16_len` helper.

## Capabilities

### New Capabilities

- `client-entity-builder`: programmatic message-entity composition.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: mention-name entities that require peer resolution.
