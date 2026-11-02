# Tasks

## 1. Loader

- [ ] 1.1 Add a YAML loader with `TELESRV_*` expansion mapping onto `EdgeConfig` (`rtd-edge/src/config.rs`)
- [ ] 1.2 Accept `--config edge.yaml` and keep the existing flags as overrides

## 2. Overlay

- [ ] 2.1 Add `deploy/docker/` compose overlay and `configs/examples/edge.local.yaml` mirroring gramsrv
- [ ] 2.2 Wire Redis configuration the edge needs

## 3. Verification

- [ ] 3.1 Verify the example config loads and maps every section (`rtd-edge` tests)
- [ ] 3.2 Verify the overlay brings the edge up against the stack and completes `help.getConfig`
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
