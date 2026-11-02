# Proposal

## Why

`help.getConfig` must advertise the edge's public address, port and DC so clients know where to
connect, and some deployments need `base_url`/`web_base_url`, a default country code and a
`strict_dc_check` toggle. `rtd-edge` advertises only a single hard-coded datacenter and port.
gramsrv's edge config carries `public.advertise`, `public.base_url`, `web_base_url`,
`default_country_code` and `strict_dc_check`.

## What Changes

- A `public` config block: advertise ip/port, base_url, web_base_url, default_country_code,
  strict_dc_check.
- `help.getConfig`/`help.getNearestDc` built from that config, with advertised DCs.
- `help_legacy` parity for older clients.

## Capabilities

### New Capabilities

- `edge-public-advert`: public advertisement in `help.getConfig`/`help.getNearestDc`.

## Impact

- **Crates**: `rtd-edge` (`config.rs`), `rtd-server` (`rpc.rs` config builders).
- **Non-goals**: multi-DC advertisement (the multi-DC spec).
