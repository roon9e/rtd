# Proposal

## Why

`td` ships fuzzing corpora (`_fuzz`: `flow`, `handle_message`, `rsa`). `rtd` has none, so the
decoder/parser surfaces are only covered by example tests. Since `rtd` parses untrusted bytes
during the exchange and message handling, fuzzing is a correctness and safety gate.

## What Changes

- Add `cargo-fuzz` targets for: the key-exchange flow, encrypted-message and container decoding,
  gzip, and the TL decoder/admission path.
- Seed the corpora with the existing interop vectors and e2e captures.
- Add a scheduled/optional CI job running each target for a bounded time.

## Capabilities

### New Capabilities

- `ci-fuzz`: fuzz targets and a bounded fuzz CI job.

## Impact

- **Crates**: `fuzz/` (new cargo-fuzz workspace), CI.
- **Non-goals**: changing protocol behavior.
