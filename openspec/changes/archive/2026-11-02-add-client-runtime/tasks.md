# Tasks

## 1. Crate and transport

- [x] 1.1 Add the `rtd-client` crate (depends on `rtd-bin`, `rtd-crypto`, `rtd-error`, `rtd-exchange`, `rtd-mt`, `rtd-proto`, `rtd-tg`, `rtd-transport`, `rtd-util`) with a `Transport` trait (`send_frame`/`recv_frame`) and an in-memory duplex implementation; verify a test moves frames through both directions (`transport.rs`: `duplex_moves_frames_both_ways`, `a_dropped_peer_reads_as_closed`)
- [x] 1.2 Add a TCP transport (codec + optional `Obfuscated2`/`FakeTls`) and a `connect_tcp` that dials, frames, and returns a `Transport`; verify against the official test server when `RTD_PEER_TESTSERVER` is set (skipped otherwise, `tests/peer_gate.rs`)

## 2. Client session

- [x] 2.1 Implement `ClientSession` over `rtd_proto::Session`: allocate a content/service message, encrypt with `rtd_crypto` (`Side::Client`), decrypt a server frame, validate the session id, and queue inbound acks; verify an encrypted round trip against a mock server that shares the auth key (`session.rs`; `tests/round_trip.rs` drives the whole path)
- [x] 2.2 Bridge `rtd_exchange::client_exchange` over a `Transport` (`Client::connect`) to establish the auth key, session id and salt from a fresh connection; verify the in-memory round trip completes the exchange and `Client` invokes afterwards (`tests/round_trip.rs::client_completes_exchange_and_invokes`)

## 3. RPC and client

- [x] 3.1 Implement the RPC engine: correlate `rpc_result` by request message id, unwrap `msg_container`/`gzip_packed`, acknowledge odd-seqno server messages with `msgs_ack`, adopt `bad_server_salt` and resend (bounded), and surface `rpc_error` as `rtd_error::Error`; verify a mock server answers an invoked request (`client.rs::handle_body`/`await_result`; `tests/round_trip.rs`)
- [x] 3.2 Implement `Client`: `connect` (dial + exchange) and `invoke(&Method) -> Result<Object>`, wrapping every call in `invokeWithLayer` at layer 232 and the first in `initConnection`; verify a `help.getConfig` round trip returns the expected `Object` (`client.rs::initialize`/`invoke`; `tests/round_trip.rs` asserts the returned `Object::Bool`)
- [x] 3.3 External-peer gate: with `RTD_PEER_TESTSERVER` set, connect to the official engine's test server over obfuscated TCP, complete the exchange, and invoke a method; verify a real reply or an `rpc_error` (skipped when unset, `tests/peer_gate.rs`)

## 4. Verification

- [x] 4.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked` including `rtd-client`'s round trip and the codegen generation-freshness tests)
