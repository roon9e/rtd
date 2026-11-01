# Tasks

Milestone 1 (foundation) only. Later stages of the migration plan
(`design.md` → Migration Plan) are not part of this change.

## 1. Workspace and tooling

- [x] 1.1 Create the cargo workspace with crates `rtd`, `rtd-bin`, `rtd-crypto`, `rtd-proto`, `rtd-transport`, `rtd-mt`, `rtd-tg`, `rtd-tlprofile`, `rtd-error`, `rtd-util`, `rtd-server`, `rtd-edge-proto`, `rtd-edge`, and the `rtd-codegen` tool; verify with `cargo metadata --format-version 1` listing every crate and `cargo build --workspace` succeeding on the skeleton
- [x] 1.2 Add `rust-toolchain.toml` (stable) and set `edition = "2024"` plus `rust-version` to the current stable in every crate; verify `cargo build` reports no edition or version errors
- [x] 1.3 Add `LICENSE-MIT`, `LICENSE-APACHE`, and a `NOTICE`/`THIRD-PARTY` file retaining the `td`/`gotd` MIT notice; verify each crate declares `license = "MIT OR Apache-2.0"` and the files exist
- [x] 1.4 Add `rustfmt.toml`, a clippy baseline that denies warnings, and `.gitignore`; verify `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass on the skeleton
- [x] 1.5 Add a CI workflow with independent checks — format, clippy, build, workspace tests, and a generation-freshness check that re-running the generator produces no diff — modelled on `td`'s CI; verify the workflow file exists and every command runs successfully locally
- [x] 1.6 Configure CI to run on both `dev` and `main`, and document the branch model (development on `dev`, releases merged to `main`) in `CONTRIBUTING.md`; verify `dev` is the current branch and the documented flow matches the workflow triggers

## 2. `rtd-bin` — TL primitive serialization

- [x] 2.1 Implement the buffer/cursor over bytes with owned and borrowed modes and a reset/copy escape hatch; verify unit tests for read, skip and copy semantics
- [x] 2.2 Implement TL primitives (int, long, double, bool, string, byte string, vector header, int128/int256) with 4-byte padding and the long string form; verify round-trip tests plus golden byte vectors
- [x] 2.3 Implement the flags bitfield, checked encoding variants, and the vector preallocation limit; verify tests including invalid-length rejection and negative vector count rejection
- [x] 2.4 Document the `rtd-bin` public API and verify `cargo doc -p rtd-bin` builds without warnings

## 3. `rtd-crypto` — MTProto cryptography

- [x] 3.1 Implement AES-IGE encryption and decryption; verify against known vectors
- [x] 3.2 Implement the MTProto 2.0 key-derivation function (message key, AES key/IV, side-dependent offset); verify client-side and server-side derivations interoperate
- [x] 3.3 Implement the auth-key type with 64-bit identifier and auxiliary hash; verify identifiers against known values and that formatting never reveals key bytes
- [x] 3.4 Implement the legacy key-derivation function and the temporary-auth-key bind envelope; verify round-trip
- [x] 3.5 Implement RSA exchange padding and its inverse plus public/private key parsing; verify round-trip with a generated key pair
- [x] 3.6 Implement Diffie-Hellman and composite validation (range checks, prime and half-prime primality, generator conditions, factorization bounds); verify against known vectors and weak-parameter rejection
- [x] 3.7 Implement SRP password hashing and proof computation including new-hash creation; verify proof accept/reject
- [x] 3.8 Implement temporary AES keys, nonce hash, and initial server salt at fixed widths; verify a leading-zero-nonce test
- [x] 3.9 Route all randomness through a cryptographically secure source and document it; verify no key material appears in `Debug`/`Display` output via a test
- [ ] 3.10 Implement the client-side auth-key exchange flow (req_pq → resPQ → req_DH_params → set_client_DH_params → dh_gen_ok) over the sans-IO core using the injected time and randomness; verify against a fake server
- [ ] 3.11 Implement the server-side auth-key exchange flow accepting both permanent and temporary key requests; verify a client-driven exchange completes and produces a matching auth key

## 4. `rtd-proto` and `rtd-error` — MTProto framing

- [x] 4.1 Implement message-id encoding, time/type recovery, and a monotonic generator; verify round-trip and collision-avoidance tests
- [x] 4.2 Implement the replay window; verify duplicate and outdated identifiers are rejected
- [x] 4.3 Implement message containers with per-element length bounds; verify oversized-element rejection and nested decode
- [x] 4.4 Implement bounded decompression with an expansion limit; verify a decompression-bomb is rejected
- [x] 4.5 Implement RPC result framing including compressed result payloads; verify request-id mapping
- [x] 4.6 Implement unencrypted exchange messages (zero auth-key id) encode/decode; verify round-trip and malformed rejection
- [x] 4.7 Implement the abridged, intermediate, padded-intermediate, and full transport codecs with headers, length encodings, padding, checksum/sequence checks, detection, and bounded chunked reads; verify byte-level goldens per codec
- [x] 4.8 Implement transport protocol-error parsing (key-not-found, wrong-DC, flood); verify each code is surfaced
- [x] 4.9 Implement obfuscated2 and FakeTLS framing with secret-based keys and DC labelling; verify goldens and DC-label extraction
- [x] 4.10 Implement the TL error type with code/message and numeric-suffix parsing plus wait-error handling; verify `TYPE_<n>` type/argument extraction and wait parsing

## 5. Schema import and generator skeleton

- [ ] 5.1 Build the import step that pulls `td`'s `_schema/layers/*.tl`, `manifest.json`, and `policy.json` into rtd while preserving each schema's tdesktop repository/commit/blob/SHA-256 provenance; verify SHA-256 verification passes and a modified schema fails the check
- [ ] 5.2 Implement the Rust TL parser for layer schema files requiring explicit constructor identifiers; verify it parses layer 232 and rejects a fixture line lacking an identifier
- [ ] 5.3 Implement the manifest and universe model (semantic families, per-layer definitions, shared wire codecs) with digest computation and SHA-256 verification; verify mismatch detection and family grouping tests
- [ ] 5.4 Implement conversion analysis, obligation digests, and policy loading; verify unreviewed differences and stale policy entries both fail generation
- [ ] 5.5 Implement route classification (`direct`/`retag`/`rewrite`/`policy`/`profile-only`/`unavailable`) over the full `(wire × profile)` product with a completeness check; verify an explicit action is required for every pair
- [ ] 5.6 Implement plan emission with digest-based deduplication (body/preflight/result); verify duplicate plans collapse to one emitted unit
- [ ] 5.7 Generate the canonical value model (`rtd-tg`) for layer 232; verify the generated crate compiles, exposes the layer constant, and provides a wire-id→name registry
- [ ] 5.8 Generate the profile sidecar (`rtd-tlprofile`) for layer 232; verify it compiles and route-lookup tests pass
- [ ] 5.9 Implement deterministic, atomic output writes and the source-budget check; verify two runs are byte-identical and that exceeding the budget fails generation
- [ ] 5.10 Generate the MTProto service schema (`rtd-mt`) from `_schema/mt.tl` (a single, non-layered schema) through the same generator; verify the generated crate compiles and provides the handshake and service types (req_pq, resPQ, req_DH_params, dh_gen_ok, ping/pong, rpc_result, msgs_ack, new_session_created, …) needed by the auth-key exchange

## 6. `rtd-tlprofile` runtime — multi-layer behavior

- [ ] 6.1 Implement a data-driven profile type and resolution with no hard-coded profile count; verify an unsupported layer resolves to "none" and is refused
- [ ] 6.2 Implement semantic identity lookup and wire-id↔semantic resolution per profile; verify identity is stable when a wire id changes between layers
- [ ] 6.3 Implement admission (exact, inherited-default, unprofiled) with wrapper parsing and admission limits; verify unsupported-layer refusal, conflicting-selector rejection, and limit enforcement
- [ ] 6.4 Implement dispatch, per-profile result encoding, and value freezing; verify an unrepresentable value fails closed and freezing enables fan-out to two profiles
- [ ] 6.5 Implement wire-invariant terminal handling; verify an invariant-only request is admitted without a profile

## 7. Edge service contract (foundation for `gramsrv` integration)

- [ ] 7.1 Define the versioned protobuf/gRPC contract for the edge boundary (session identity, admitted-call metadata, canonical request and result as opaque TL bytes, lifecycle and routing events, key/session lookups, error propagation); verify the contract compiles and an admitted-call plus result round-trip test passes
- [ ] 7.2 Add the `rtd-edge-proto` and `rtd-edge` crate skeletons with generated contract types committed; verify `cargo build -p rtd-edge-proto -p rtd-edge` succeeds and a version-mismatch handshake is rejected in a test

## 8. Interop harness

- [x] 8.1 Add a Go helper that uses `td` to emit shared crypto and framing vectors; verify it runs and writes a vector file
- [x] 8.2 Add Rust tests that consume the shared vectors; verify a deliberately altered expected value fails the test
- [x] 8.3 Add an injectable clock to `rtd-util`; verify a test that advances the clock changes time-dependent behavior without real waiting
- [x] 8.4 Add an injectable random source to the protocol core; verify a fixed source produces byte-identical output across runs
- [ ] 8.5 External-peer gate: obtain and build the official engine's test server (and its C-ABI library where needed), drive it with a minimal rtd client path (transport + codec + obfuscated2 + auth-key exchange + service ping) and require the handshake and a pong to succeed; verify no crate or build step depends on the official engine's source, and record the gate result

## 9. Integration verification

- [ ] 9.1 Verify codec-level cross-implementation compatibility: bytes produced by the Go `td` helper decode in Rust and bytes produced by Rust decode in Go; the interop test passes
- [ ] 9.2 Run the full workspace gate (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and confirm it is green
- [ ] 9.3 Add a top-level `README.md` and a short `docs/architecture.md` describing the crate layout, generation workflow, edge-service boundary, and core decisions, and verify the documented regeneration command runs as written
- [ ] 9.4 Run the official-engine external-peer gate (task 8.5) and the Go `td` interop (task 9.1) as required checks and confirm both are green; if the peer cannot be fetched in CI, provide and run the documented local command

## Workflow follow-up

- Archive this change after the project's review requirements are satisfied, then verify the archived result.
- Push `dev` to the `roon9e` remote and confirm CI runs; release by merging `dev` into `main` (D13).
- Implement the edge service itself (beyond the contract skeleton) in migration-plan stage 3 (D14).
- Add the first-party PostgreSQL storage backend at the multi-DC/outbox stage (D12).
- Realize the durable outbox `Delivery` backend at migration-plan stage 4 (D10).
- Make the FakeTLS ClientHello byte-accurate to a real Chrome fingerprint (the current template is a minimal, valid TLS handshake that satisfies the HMAC contract but is not a uTLS-identical fingerprint).
- Re-baseline the multi-DC design (D5, D9, D10, D15, D16) against the maintainer's v2 multi-DC implementation once it is available in a public branch/commit.
- Decide the two candidate adoptions from the official-engine review: `zeroize` for key material in `rtd-crypto`, and a security-requirement-to-test matrix.
- Review the sans-IO core boundary (D18) against the official engine's `mtproto-core` module layout and record any further alignment.
- Install Go (for the `td` vector helper, tasks 8.1/9.1) and confirm the external peer (task 8.5) can be fetched/built in CI.
