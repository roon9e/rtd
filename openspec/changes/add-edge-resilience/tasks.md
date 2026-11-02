# Tasks

## 1. Keep listening

- [x] 1.1 `Server::serve` continues accepting after a per-connection accept error (bounded backoff) instead of returning; verify a bad connection does not stop the server (`actor::serve_survives_a_bad_connection`)
- [x] 1.2 `build_core` uses a lazy core channel so a temporarily unavailable core does not stop the edge from starting and listening (`service.rs`; `endpoint.connect_lazy()`)

## 2. Live verification

- [x] 2.1 Rebuild `rtd-edge`, restart it in the stack, and confirm it stays up across the core's startup and that a client probe that closes early does not kill it (edge `restarts=0` over 25 s and across a connect-and-close; port stays open)

## 3. Verification

- [x] 3.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
