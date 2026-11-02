# Proposal

## Why

The edge has a transient session registry, but gramsrv models logical sessions with lifecycle
events: provisional revocation, session barriers on shutdown, quick-ack deadlines, pending flush
and `destroy_auth_key`. These make reconnects and shutdowns safe under load
(`logical_session.go`, `session_manager.go`, `shutdown_gate_test.go`, `quick_ack_deadline_test.go`).

## What Changes

- A logical session with a lifecycle (created/ready/revoked) and `new_session_created` handling.
- Provisional revocation: a key used before confirmation is revoked if never confirmed.
- A shutdown gate + session barrier that drains in-flight work before exit.
- A quick-ack deadline and pending flush so unacknowledged messages are not lost.

## Capabilities

### New Capabilities

- `edge-session-lifecycle`: logical-session lifecycle, revocation and graceful shutdown.

## Impact

- **Crates**: `rtd-server` (`session.rs`, `actor.rs`), `rtd-edge` (shutdown).
- **Non-goals**: DC-level session routing (the multi-DC spec).
