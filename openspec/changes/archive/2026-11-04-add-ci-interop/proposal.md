# Proposal

## Why

`openspec/config.yaml` requires CI to run an **interop** check and treats
`TelegramMessenger/mtproto-engine` as a first-class wire-compatibility target, but
`.github/workflows/ci.yml` has only fmt, clippy, build+test and generation-freshness. The
external-peer gate (`RTD_PEER_TESTSERVER`) exists in tests but is never exercised in CI.

## What Changes

- Add an interop CI job that builds the official Rust engine's `mtproto-testserver` (GPL-2.0,
  never linked or vendored) and runs the `peer_gate` tests in `rtd-exchange` and `rtd-client`
  against it.
- Add the Go cross-check vector job (regenerate `interop/vectors.json` and run the binary tests).
- Run the client -> edge -> core drop-in integration as an automated job.

## Capabilities

### New Capabilities

- `ci-interop`: automated interoperability and external-peer checks.

## Impact

- **Files**: `.github/workflows/ci.yml` (new jobs), `interop/`.
- **Non-goals**: publishing images (separate workflow).
