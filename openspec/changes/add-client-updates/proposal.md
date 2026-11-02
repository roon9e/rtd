# Proposal

## Why

A server pushes updates (new messages, config changes, …) as messages that are not the answer
to a request. `rtd-client` currently discards any inbound message that is not an `rpc_result`,
so a caller cannot observe them.

## What Changes

- `Client::set_update_handler`: a callback invoked for each `Updates` envelope (or bare
  `Update`) the server sends in-band while a call is in flight; a bare `Update` is wrapped in
  `Updates::updateShort`.
- A mock-server test that sends an `updateShort` before the `rpc_result` and asserts the
  handler observed it.

## Capabilities

### New Capabilities

- `client-updates`: dispatch of server update messages to a caller-provided handler.

## Impact

- **Crates**: `rtd-client` only; no wire changes.
- **Non-goals**: a background receive loop (updates are delivered in-band during calls), gap
  recovery/state, and routing updates to the async façade's caller.
