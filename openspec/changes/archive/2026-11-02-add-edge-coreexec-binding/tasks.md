# Tasks

## 1. Dispatch binding

- [x] 1.1 Add `auth_key_id: AuthKeyId` and `msg_id: i64` to `AdmittedCall` and fill them in the connection actor; update the in-process and edge round-trip tests; verify the round trips still pass (the mock core now rejects an all-zero id, so the binding is exercised)
- [x] 1.2 Send `auth_key_id` and `msg_id` in `GrpcCore`'s `DispatchRequest`; verify via the in-process tonic test server that the core receives them (`dispatch_without_auth_key_id_fails`, `dispatch_round_trips_over_grpc`)

## 2. Negotiated layer and wire proof

- [x] 2.1 Forward the client's negotiated layer, not the edge's canonical default: the actor derives the call profile from the admission's explicit/effective profile, and the adapter re-admits in default mode (`dispatch_content`; live: a layer-229 client is admitted at 229)
- [x] 2.2 Carry the wire proof on `AdmittedCall` (`wire_size`, `wire_digest`, `wire_invariant`, `effective_profile`, `profile_evidence`) and transmit it in `DispatchRequest.proof`; verify against the live core (`admitted wire proof mismatch` is gone)
- [x] 2.3 Make `is_invariant_terminal` mirror the reference generator: compare the per-layer preflight and result plans (not the result type name), so `auth.sendCode` is no longer over-marked; the set now has 400 entries, matching `td` (`emit_profile::emit_invariant`; `getConfig` invariant, `sendCode`/`signIn` not)

## 3. Offline report binding

- [x] 3.1 Add the authorization-key id to `Core::report_session_offline` and send it as `SessionOfflineEvent.raw_auth_key_id`; verify the actor and adapter compile

## 4. Live end-to-end

- [x] 4.1 Rebuild `rtd-edge` and run it as a drop-in in the gramsrv v2 stack; verify the login flow (`help.getConfig` -> `auth.sendCode` -> `auth.signIn`) succeeds (`E2E OK` against the prebuilt `core:v2` at layer 229)

## 5. Verification

- [x] 5.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green (`GEN_OK`/`FMT_OK`/`CLIPPY_OK`/`TEST_OK`; the regenerated sidecar is 395,067 lines)
