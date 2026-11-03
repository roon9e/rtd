# Proposal

## Why

`td` generates two extra schemas beyond the canonical API: `_schema/encrypted.tl` (secret chats)
and `_schema/trace.tl` (tracing). `rtd-codegen` emits only the canonical `rtd-tg`, `rtd-mt` and
`rtd-tlprofile`, so secret-chat messages and trace objects cannot be encoded or decoded.

## What Changes

- Extend `rtd-codegen` to emit the secret-chat schema as a generated crate/module
  (`rtd-tg-e2e`-equivalent): `DecryptedMessage`, media, actions, `DocumentAttribute`, selectors.
- Extend it to emit the tracing schema (`tgtrace`-equivalent).
- Import `_schema/encrypted.tl` and `_schema/trace.tl` following the existing manifest
  provenance rules.

## Capabilities

### New Capabilities

- `secret-chats`: generated end-to-end (secret chat) value model.
- `tl-tracing`: generated tracing value model.

## Impact

- **Crates**: `rtd-codegen` (new emit paths), new generated crates, `rtd-tlprofile` untouched.
- **Non-goals**: the secret-chat runtime (session/MTProto layer above the schema).
