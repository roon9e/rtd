# Design

## Context

`rtd-server::rpc` already builds `help.getConfig`/`help.getNearestDc` from `ServerConfig`
(`dc`, `datacenters`). The edge hard-codes one datacenter at `127.0.0.1:443`. gramsrv's config
supplies richer public fields.

## Goals / Non-Goals

**Goals**

- Correct, configurable advertisement so real clients connect to the right host.
- Optional base URLs and country code.

**Non-Goals**

- Multi-DC advertisement; server-info.

## Decisions

### D1. A `PublicConfig` type

`PublicConfig { dc, advertise_ip, advertise_port, base_url, web_base_url, default_country_code,
strict_dc_check }` feeds `ServerConfig`, replacing the hard-coded datacenter.

### D2. `strict_dc_check` reached the exchange already

The exchange's non-strict DC behavior (from `add-edge-nonstrict-dc`) is driven by this flag.

### D3. `help_legacy`

Older clients expecting legacy config fields are served a compatible `help.getConfig`, mirroring
gramsrv's `help_legacy.go`.

## Risks / Trade-offs

- [Wrong advertise IP breaks clients] → validate the address and default to the bind address.
