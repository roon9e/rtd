# Tasks

## 1. Dependencies and scaffolding

- [ ] 1.1 Add `tokio`, `tonic`, `prost` and `tokio-tungstenite` as dependencies of `rtd-server`, `rtd-edge` and `rtd-edge-proto` only, leaving the sans-IO crates' dependency sets unchanged; verify `cargo tree -p rtd-proto -p rtd-mt -p rtd-tlprofile` lists none of them and `cargo build --workspace` succeeds
- [ ] 1.2 Create the `rtd-proto` `encrypted`/`session` modules and the `rtd-server` module skeleton (`storage`, `transport`, `session`, `rpc`, `core`), each re-exported from `lib.rs` with crate docs and no lint allowances; verify `cargo build -p rtd-proto -p rtd-server` and `cargo clippy -p rtd-proto -p rtd-server --all-targets -- -D warnings` succeed

## 2. `rtd-proto` — encrypted framing and server salt

- [ ] 2.1 Implement the encrypted message envelope (auth-key id, message key, AES-IGE payload) with encode/decode that recomputes the message key and rejects a mismatch, distinguishing a zero auth-key id as plaintext; verify round-trip and tamper-rejection unit tests
- [ ] 2.2 Implement server-salt handling (derive from the exchange nonces, track the current salt, replace it on a new salt) and expose it to the session layer; verify a leading-zero-nonce golden and a salt-replacement test
- [ ] 2.3 Document the encrypted/salt module APIs; verify `cargo doc -p rtd-proto` builds without warnings

## 3. `rtd-proto` — sans-IO session state machine

- [ ] 3.1 Implement session message-id and sequence allocation using the injected clock and randomness, distinguishing content from service messages; verify the content-rule sequence test
- [ ] 3.2 Implement the inbound replay window and duplicate tolerance for encrypted messages; verify out-of-window rejection and duplicate-retransmission tolerance tests
- [ ] 3.3 Implement pending-ack tracking and the resend queue (record outbound content messages, answer a resend request, drop on ack); verify ack-answer and resend tests
- [ ] 3.4 Implement session identity and lifecycle state (session id, salt, layer slot) with clock-driven expiry and disconnect handling; verify a session-expiry test using `ManualClock`

## 4. `rtd-server` — storage traits and in-memory backend

- [ ] 4.1 Define `AuthKeyStore` and `SessionStore` traits with `(dc, key_id)` and `(auth_key_id, session_id)` scoping plus temporary-key expiry; verify an implementation of each compiles and doc tests describe the contract
- [ ] 4.2 Implement the in-memory backend with fail-closed writes; verify store/get/expire/remove tests and a failure-injection test that a failed write does not confirm the operation

## 5. `rtd-server` — ingress transports

- [ ] 5.1 Implement plain-TCP ingress with framing detection (abridged, intermediate, padded-intermediate, full) behind the `Ingress` abstraction that yields a framed stream plus metadata; verify one detection test per framing over an in-memory duplex
- [ ] 5.2 Implement obfuscated2 and FakeTLS/MTProxy ingress with secret authentication and DC-label extraction; verify accept and wrong-secret-reject tests
- [ ] 5.3 Implement WebSocket ingress carrying the MTProto stream; verify a handshake plus frame round-trip test
- [ ] 5.4 Document the ingress API and supported transports in `docs/architecture.md`; verify the documented transport list matches the implemented ones

## 6. `rtd-server` — connection actor, sessions and service messages

- [ ] 6.1 Implement the per-connection actor with a single writer task that assigns ids/seq and serializes writes; verify a test that interleaved outbound messages keep monotonic ids and correct ordering
- [ ] 6.2 Wire the server side of the key exchange into the actor, persisting the key before confirming, and accept encrypted frames by auth-key id, answering an unknown id with auth-key-not-found; verify exchange-then-encrypted-frame and unknown-key tests
- [ ] 6.3 Implement session lifecycle (`new_session_created` once, release on disconnect) and service messages (`ping`/`pong`/`ping_delay_disconnect`, `msgs_ack`, future salt, resend, bad-message/bad-salt, `destroy_session`); verify one end-to-end service-message test per class
- [ ] 6.4 Implement admission budgets (total connections, per source, concurrent handshakes, aggregate frame bytes) with load-shedding that does not disturb admitted sessions; verify limit-refusal and admitted-session-continuity tests

## 7. `rtd-server` — RPC dispatch, router, layer binding and built-ins

- [ ] 7.1 Implement invocation-wrapper unwrapping with the depth bound and initialization-info capture; verify depth-limit rejection and info-capture tests
- [ ] 7.2 Implement per-session layer binding (explicit select, inherited default, ordered correction, fail-closed unsupported) with per-profile result encoding via `rtd-tlprofile`; verify the inherited-default, ordered-correction and fail-closed tests
- [ ] 7.3 Implement the `Router` trait, result-identity binding, error mapping and unknown-method handling; verify custom-router handling, mismatched-result rejection and unknown-method tests
- [ ] 7.4 Implement the built-ins `help.getConfig` and `help.getNearestDc`; verify each returns the configured datacenter set and the serving datacenter

## 8. Core boundary — trait and gRPC adapter

- [ ] 8.1 Define the `Core` trait and an in-process implementation over the router, mapping a core failure to a TL error while keeping the session alive; verify in-process execute and core-failure tests
- [ ] 8.2 Add the call/event messages the boundary needs to `rtd-edge-proto` and bump the contract version if required; verify prost round-trip and version-mismatch rejection tests
- [ ] 8.3 Implement the tonic gRPC adapter that dials the core's `EdgeCore.Execute`; verify a tonic in-process test that an admitted call reaches the core and its result is encoded

## 9. `rtd-edge` service binary

- [ ] 9.1 Wire `rtd-edge` to construct and run the edge from configuration (bind address, proxy secret, RSA key, core endpoint or in-process) until shutdown; verify a smoke test that the binary starts, accepts a connection and exits cleanly
- [ ] 9.2 Document the edge configuration and sidecar run command in `README.md`; verify the documented command runs as written against an in-process core

## 10. Integration verification

- [ ] 10.1 In-process end-to-end: a test client completes a key exchange and calls a built-in method against a spawned rtd edge over each TCP framing; verify the test passes
- [ ] 10.2 In-process end-to-end over MTProxy (obfuscated2 and FakeTLS) and WebSocket; verify the tests pass
- [ ] 10.3 Extend the official-engine external-peer gate to a full encrypted session (exchange plus an encrypted ping answered by pong); verify the gate passes locally and names the divergence on failure
- [ ] 10.4 Run the full workspace gate (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and the generation-freshness check, and confirm all are green

## Workflow follow-up

- Archive this change after the project's review requirements are satisfied, then verify the archived result.
- Stage 4 (multi-DC: registry, routing, migration, authorization transfer, delivery) follows as its own change.
- The parallel generator stage (layers 225–231 and non-direct route bodies) follows as its own change.
