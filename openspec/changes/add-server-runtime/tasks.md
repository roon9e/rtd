# Tasks

## 1. Dependencies and scaffolding

- [x] 1.1 Add `tokio`, `tonic`, `prost` and `tokio-tungstenite` as dependencies of `rtd-server`, `rtd-edge` and `rtd-edge-proto` only, leaving the sans-IO crates' dependency sets unchanged; verify `cargo tree -p rtd-proto -p rtd-mt -p rtd-tlprofile` lists none of them and `cargo build --workspace` succeeds
- [ ] 1.2 Create the `rtd-crypto` `encrypted_message`/`salt` modules, the `rtd-proto` `session` module, and the `rtd-server` module skeleton (`transport`, `session`, `rpc`, `core`), each re-exported from `lib.rs` with crate docs and no lint allowances; verify `cargo build -p rtd-crypto -p rtd-proto -p rtd-server` and `cargo clippy -p rtd-crypto -p rtd-proto -p rtd-server --all-targets -- -D warnings` succeed
- [x] 1.3 Re-audit the committed crates against the gramsrv v2 edge dependencies (`iamxvbaba/td/{bin,clock,crypto,exchange,mt,mtproxy,proto,tgerr,tg,tlprofile,tmap,transport}`) and record any further gap as a task before implementing the group it affects; verify every listed package has an rtd owner or a task in this list

## 2. Multi-layer runtime extension (`rtd-tlprofile` and `rtd-codegen`)

The committed runtime covers admission/freeze/dispatch but not the wrapper and call surface the edge needs (D28).

- [x] 2.1 Extend the generator to emit a wrapper-aware admission surface — capture of initialization-wrapper client/connection info and the invoke-after ordering identifiers, plus the call identity (semantic, wire id, ordering key); verify generation is deterministic and the committed output is refreshed
- [x] 2.2 Extend the `rtd-tlprofile` runtime with an admission view (explicit and effective profile plus wrapper metadata) and a dispatch `Result` carrying the admitted call identity; verify initialization-wrapper capture, invoke-after ordering and result-identity tests
- [x] 2.3 Re-verify the existing multi-layer behavior is unchanged (`unsupported-layer refusal`, conflicting-selector rejection, fail-closed encoding) with `cargo test -p rtd-tlprofile`, and confirm `cargo run -p rtd-codegen -- verify` passes

## 3. `rtd-crypto` — encrypted message envelope and server salt

- [x] 3.1 Implement the encrypted message envelope and inner header (salt, session id, message id, sequence, body) with encrypt/decrypt that recomputes the message key and rejects a mismatch, distinguishing a zero auth-key id as plaintext; verify round-trip and tamper-rejection unit tests
- [x] 3.2 Implement server-salt derivation from the exchange nonces by the MTProto rule and expose the inner-header encode/decode; verify a leading-zero-nonce golden and a both-sides-agree test
- [x] 3.3 Document the encrypted-message and salt APIs; verify `cargo doc -p rtd-crypto` builds without warnings

## 4. `rtd-proto` — sans-IO session state machine

- [ ] 4.1 Implement the session struct with message-id and sequence allocation (adding `rtd-crypto` as a dependency of `rtd-proto` for the encrypted-message types) using the injected clock and randomness and distinguishing content from service messages; verify the content-rule sequence test
- [ ] 4.2 Implement the inbound replay window and duplicate tolerance for encrypted messages; verify out-of-window rejection and duplicate-retransmission tolerance tests
- [ ] 4.3 Implement pending-ack tracking and the resend queue (record outbound content messages, answer a resend request, drop on ack); verify ack-answer and resend tests
- [ ] 4.4 Implement session identity and lifecycle state (session id, salt, layer slot) with clock-driven expiry and disconnect handling; verify a session-expiry test using `ManualClock`

## 5. `rtd-transport` and config compatibility

- [x] 5.1 Add MTProxy secret parsing (human string to obfuscation mode and key) covering plain, secured (`dd`) and FakeTLS (`ee`) secrets; verify parse and wrong-mode rejection tests
- [x] 5.2 Add a combined wire-id to name registry over the `rtd-tg`, `rtd-mt` and `rtd-proto` registries (the `td/tmap` equivalent); verify lookups across all three and that an unknown id returns none

## 6. `rtd-server` — edge session state and core-backed durable state

- [ ] 6.1 Implement the transient session registry and the auth-key cache keyed by authorization-key id, with clock-driven eviction on disconnect; verify register/lookup/evict tests
- [ ] 6.2 Implement durable-state access through the core abstraction (save a newly created key before confirming, fetch a key or layer evidence the edge does not hold), failing closed when the core cannot persist; verify save-before-confirm and fetch-on-unknown tests

## 7. `rtd-server` — ingress transports

- [ ] 7.1 Implement plain-TCP ingress with framing detection (abridged, intermediate, padded-intermediate, full) behind the `Ingress` abstraction that yields a framed stream plus metadata, including the same-port plaintext/obfuscated discrimination used by the edge; verify one detection test per framing over an in-memory duplex
- [ ] 7.2 Implement obfuscated2 and FakeTLS/MTProxy ingress with secret authentication and DC-label extraction; verify accept and wrong-secret-reject tests
- [ ] 7.3 Implement WebSocket ingress carrying the MTProto stream; verify a handshake plus frame round-trip test
- [ ] 7.4 Document the ingress API and supported transports in `docs/architecture.md`; verify the documented transport list matches the implemented ones

## 8. `rtd-server` — connection actor, sessions and service messages

- [ ] 8.1 Implement the per-connection actor with a single writer task that assigns ids/seq and serializes writes; verify a test that interleaved outbound messages keep monotonic ids and correct ordering
- [ ] 8.2 Wire the server side of the key exchange into the actor, saving the key to the core before confirming, and accept encrypted frames by auth-key id, resolving or rejecting an unknown id through the core; verify exchange-then-encrypted-frame and unknown-key tests
- [ ] 8.3 Implement session lifecycle (`new_session_created` once, release on disconnect) and service messages (`ping`/`pong`/`ping_delay_disconnect`, `msgs_ack`, future salt, resend, bad-message/bad-salt, `destroy_session`); verify one end-to-end service-message test per class
- [ ] 8.4 Implement admission budgets (total connections, per source, concurrent handshakes, aggregate frame bytes) with load-shedding that does not disturb admitted sessions; verify limit-refusal and admitted-session-continuity tests

## 9. `rtd-server` — RPC dispatch, router, layer binding and built-ins

- [ ] 9.1 Implement invocation-wrapper unwrapping with the depth bound, initialization-info capture and invoke-after ordering using the extended `rtd-tlprofile` surface; verify depth-limit rejection, info-capture and ordering tests
- [ ] 9.2 Implement per-session layer binding (explicit select, inherited default, ordered correction, fail-closed unsupported) with per-profile result encoding; verify the inherited-default, ordered-correction and fail-closed tests
- [ ] 9.3 Implement the `Router` trait, result-identity binding, error mapping and unknown-method handling; verify custom-router handling, mismatched-result rejection and unknown-method tests
- [ ] 9.4 Implement the built-ins `help.getConfig` and `help.getNearestDc`; verify each returns the configured datacenter set and the serving datacenter

## 10. Core boundary — trait, in-process core and `CoreExecService`-shaped gRPC adapter

- [ ] 10.1 Define the `Core` trait mirroring gramsrv v2's edge-facing handler (admit/dispatch, durable auth keys, layer evidence, session offline) with an in-process implementation over the router, mapping a core failure to a TL error while keeping the session alive; verify in-process dispatch and core-failure tests
- [x] 10.2 Reshape `rtd-edge-proto` to mirror gramsrv v2's `coreexec.proto` — package `telesrv.coreexec.v1`, service `CoreExecService`, the method names and `DispatchRequest`/`DispatchResponse`/auth-key/layer-evidence messages with matching field tags, and a protocol-version handshake — importing the `.proto` as a pinned, provenance-recorded input rather than a hand copy; verify prost round-trip, version/capability handshake, and that the pinned provenance digest matches
- [ ] 10.4 Verify containment: only `rtd-edge` depends on `rtd-edge-proto`, and no sans-IO crate or `rtd-server` references the contract types; verify with `cargo metadata`/`cargo tree` and a source scan for the contract symbols
- [ ] 10.3 Implement the tonic gRPC adapter that dials the core's `CoreExecService`; verify a tonic in-process test that an admitted call reaches the core and its result is encoded

## 11. `rtd-edge` service binary

- [ ] 11.1 Wire `rtd-edge` to construct and run the edge from configuration (bind address, proxy secret, RSA key, core endpoint or in-process) until shutdown; verify a smoke test that the binary starts, accepts a connection and exits cleanly
- [ ] 11.2 Document the edge configuration and sidecar run command in `README.md`; verify the documented command runs as written against an in-process core

## 12. Integration verification

- [ ] 12.1 In-process end-to-end: a test client completes a key exchange and calls a built-in method against a spawned rtd edge over each TCP framing; verify the test passes
- [ ] 12.2 In-process end-to-end over MTProxy (obfuscated2 and FakeTLS) and WebSocket; verify the tests pass
- [ ] 12.3 Extend the official-engine external-peer gate to a full encrypted session (exchange plus an encrypted ping answered by pong); verify the gate passes locally and names the divergence on failure
- [ ] 12.4 Run the full workspace gate (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and the generation-freshness check, and confirm all are green

## Workflow follow-up

- Archive this change after the project's review requirements are satisfied, then verify the archived result.
- Stage 4 (multi-DC: registry, routing, migration, authorization transfer, delivery) follows as its own change.
- The parallel generator stage (layers 225–231 and non-direct route bodies) follows as its own change.
