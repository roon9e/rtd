# Tasks

## 1. Core-exec bearer token

- [x] 1.1 Add `core_token: Option<String>` to `EdgeConfig` (`--core-token` / `RTD_EDGE_CORE_TOKEN`) and pass it to `GrpcCore`; verify config parsing (flag, env, inline) and that it is optional (`config.rs`: `parses_explicit_options`, `falls_back_to_environment`; `service.rs` passes it through)
- [x] 1.2 Attach `authorization: Bearer <token>` to every `GrpcCore` unary request when a token is set; verify the request carries the header and that an untokened call carries none (`grpc.rs`: `apply_bearer`; tests `apply_bearer_sets_and_omits_token`, `core_exec_token_is_presented_as_bearer_metadata`, `no_token_sends_no_authorization_metadata`)

## 2. Container packaging

- [x] 2.1 Add a `Dockerfile` building `rtd-edge` and running it non-root (built and tagged `rtd-edge:local` against a live gramsrv v2 core — see the handoff)

## 3. Verification

- [x] 3.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green (verified together with `add-edge-coreexec-binding`; the edge container was additionally run live against a gramsrv v2 core)
