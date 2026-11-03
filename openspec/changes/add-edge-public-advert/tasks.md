# Tasks

## 1. Config

- [x] 1.1 Add a `public` config block (advertise, base URLs, country code, strict_dc_check) (`rtd-edge/src/config.rs`)
- [x] 1.2 Feed `ServerConfig` from it, replacing the hard-coded datacenter (`rtd-edge/src/service.rs`)

## 2. Methods

- [x] 2.1 Build `help.getConfig`/`help.getNearestDc` from the public config with advertised DCs (`rtd-server/src/rpc.rs`)
- [ ] 2.2 Add `help_legacy` compatibility

## 3. Verification

- [x] 3.1 Verify advertised address/port/DC appear in `help.getConfig` (`rtd-server`/`rtd-edge` tests)
- [x] 3.2 Verify `strict_dc_check` drives the exchange behavior
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
