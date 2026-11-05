# Proposal

## Why

Telegram methods can return a bare `Vector<T>` (e.g. `users.getUsers`, `messages.getMessages`).
`Client::invoke` decodes only `Object`, so those methods are unusable and `Client::Self` cannot
be implemented. `td` models this with `tg.Vector` as a `TLObject`.

## What Changes

- Add `InvokeResult` (`Object(Object)` | `Vector(Vec<Object>)`) and `Client::invoke_raw`.
- Teach `decode_result` to recognize the `vector` magic `0x1cb5c415`.
- Keep `Client::invoke` returning `Object`, failing on a vector result.

## Capabilities

### New Capabilities

- `client-vector-results`: bare-vector RPC results.

## Impact

- **Crates**: `rtd-client`.
- **Non-goals**: changing the existing `invoke` signature.
