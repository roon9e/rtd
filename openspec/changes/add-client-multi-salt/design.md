# Design

## Context

`rtd_proto::Session` and `rtd-client::ClientSession` carry one `salt`. `rtd-mt` already defines
`Get_future_salts` and the `future_salts` service object, and `rtd-server::actor` already answers
`get_future_salts` (currently reusing the current salt). `td` stores multiple salts with expiry
and refreshes before the newest expires.

## Goals / Non-Goals

**Goals**

- Multiple salts with expiry, selected deterministically by the injected clock.
- Proactive refresh and correct handling of the `future_salts` service reply.

**Non-Goals**

- Changing the server's salt policy; only the client store.

## Decisions

### D1. A store keyed by `valid_until`

`SaltStore { entries: Vec<(i64, i64)> }` with `get(now)` (newest entry not yet expired),
`insert(valid_until, salt)`, and `reset(salt)` for `new_session_created`. Selection is
`max_by_key(valid_until)` among unexpired entries, matching `td`.

### D2. Refresh at a fraction of the remaining lifetime

When `now >= valid_until - refresh_margin`, send `get_future_salts` once (in-flight guarded).
The reply is a service message, so the reader must classify `future_salts` before `rpc_result`.

### D3. Bad salt adopts the newest

On `bad_server_salt`, adopt the salt named in the notification, resend the unacknowledged
message, and drop any later-expired entry so selection converges.

## Risks / Trade-offs

- [Clock skew] → all time comes from `rtd_util::Clock`, so tests are deterministic with
  `ManualClock`.
