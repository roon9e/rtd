# Design

## Context

gramsrv v2's `coreexec` server resolves the call's authorization key from
`DispatchRequest.auth_key_id` (and validates the session/message binding) before admitting and
executing. `rtd-server`'s edge-facing `AdmittedCall` was designed for the in-process
`LocalCore`, which needs only the session and the canonical request, so it never carried the
auth-key id or message id. The remote `GrpcCore` adapter could only send defaults. The same gap
affects `ReportSessionOffline`.

## Goals / Non-Goals

**Goals**

- Send the auth-key id (and message id) that bind an admitted call, and the auth-key id of an
  offline session, so a real core accepts both.
- Keep `LocalCore` and the in-process tests unchanged in behavior.

**Non-Goals**

- Admission sequencing, profile-evidence freshness, and client-IP plumbing.

## Decisions

### D1. Extend `AdmittedCall`; do not add a second call type

`AdmittedCall` gains `auth_key_id: AuthKeyId` and `msg_id: i64`. The actor already holds both
(`self.auth_key_id`, `header.msg_id`) when it dispatches, so the fields are filled at the one
production construction site; `LocalCore` ignores them. This keeps a single call type across the
in-process and remote cores.

### D2. `report_session_offline` takes the auth-key id

The trait method becomes `report_session_offline(&self, auth_key_id: AuthKeyId, session:
SessionId)`. The actor's teardown already has both values, and the remote adapter needs the id
to fill `SessionOfflineEvent.raw_auth_key_id`.

### D3. Remaining binding fields stay at their protocol defaults

`admission_seq`, `profile_evidence_fresh`, `identity_hint` and `client_ip` are left as their
zero/default values; the canonical-layer path (one layer, one DC) does not require them, and
adding them would widen the trait without a caller that needs them yet.

### D4. Forward the negotiated layer, and match the core's admission mode

The edge's own canonical layer is 232, but a real gramsrv v2 core pins `td v1.7.0` whose canonical
is 229, and the core rejects an unsupported exact profile before it even reads the wire. The
actor forwards the layer the client negotiated (the admission's explicit/effective profile) and,
because it admits with default inheritance (`admit_default_with_limits`), the adapter re-admits
in `AdmissionMode::Default`, exactly as gramsrv's own edge does.

### D5. Recompute the wire proof the core will check

The core recomputes a proof from its own re-admission and requires equality. The edge therefore
carries the proof on `AdmittedCall` and transmits it: the wire size and a lowercase-hex SHA-256 of
the wire bytes, the wire-invariance flag, and the effective-profile and profile-evidence marks
(value = the negotiated profile, presence from the wrapper evidence). `LocalCore` ignores them.

### D6. Correct the generated wire-invariance predicate

The core's proof includes `tlUnprofiledInvariant`, which `td` computes from the per-layer
preflight and result plans. rtd's `emit_invariant` compared only the result *type* name, which
over-marks a method whose fields changed across layers (`auth.sendCode`) and produced a proof
mismatch. It now compares the per-layer preflight and result plans (and the wire id, and requires
a direct route), yielding the same 400-method set as `td`.
