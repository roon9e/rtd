# Tasks

## 1. Non-strict DC

- [x] 1.1 Add `strict_dc: bool` to `ServerConfig`; the exchange accepts a foreign DC label when it is false and rejects it when true; verify both (`non_strict_accepts_a_foreign_datacenter`, `wrong_datacenter_is_rejected`)
- [x] 1.2 `rtd-edge` sets `strict_dc: false`; verify the live client completes the handshake (decoded capture: the client's DC 1/2/3 `req_DH_params` now reach `server_DH_params_ok` + `dh_gen_ok`; before, those DCs got only `resPQ`)
- [x] 1.3 Record the multi-DC caveat (label is routing identity; clients may use labels far above 5) in the `multi-dc` spec and the handoff

## 2. Verification

- [ ] 2.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
