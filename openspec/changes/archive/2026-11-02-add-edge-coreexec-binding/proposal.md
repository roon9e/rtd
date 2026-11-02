# Proposal

## Why

Running `rtd-edge` in front of a real gramsrv v2 core, the authorization-key exchange and
`SaveAuthKey` succeed (the edge presents the core-exec bearer token), but every admitted call is
rejected. Three boundary facts the core validates were missing from the edge's `DispatchRequest`:

1. `auth_key_id` / `msg_id` — the call's binding. The edge sent an empty id and `msg_id: 0`, so
   the core answered `InvalidArgument: bad auth_key_id`.
2. the **negotiated layer** — the edge always declared its own canonical layer (232), but the
   running core's canonical is layer 229 (it pins `td v1.7.0`), so the core answered
   `unsupported exact profile 232`.
3. the **wire proof** — the core recomputes a proof from its own re-admission and requires the
   edge's proof to equal it; the edge sent an empty digest and no profile marks, so the core
   answered `admitted wire proof mismatch`.

`ReportSessionOffline` was likewise rejected for a missing raw auth key.

## What Changes

- `AdmittedCall` carries the call binding (`auth_key_id`, `msg_id`), the negotiated layer, and the
  wire-proof fields (`wire_size`, `wire_digest`, `wire_invariant`, `effective_profile`,
  `profile_evidence`); the connection actor fills them from its admission.
- `Core::report_session_offline` gains the authorization-key id.
- The remote adapter transmits all of them, and re-admits in the mode the edge used (default
  inheritance).

## Capabilities

### Modified Capabilities

- `edge-service`: the edge produces a dispatch request a real gramsrv v2 core accepts.

## Impact

- **Crates**: `rtd-server` (`core::AdmittedCall`, `Core::report_session_offline`, the actor) and
  `rtd-edge` (the adapter and tests).
- **Non-goals**: the edge still leaves admission sequencing (`admission_seq`), profile-evidence
  freshness and the edge-observed client IP at their defaults.
- **Compatibility**: with the in-process `LocalCore`, behavior is unchanged (it ignores the new
  fields).
