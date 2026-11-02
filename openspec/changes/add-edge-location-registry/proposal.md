# Proposal

## Why

Telegram maps a client's IP to a country/region (`help.getConfig` DC hints, geo-routing) and
tracks edge locations with heartbeats. gramsrv's edge has a location registry
(`internal/mtprotoedge/location_registry.go`) backed by GeoIP (`internal/geoip`) and a node
heartbeat. `rtd-edge` has none.

## What Changes

- A GeoIP-backed client-location provider (country/region from the client IP), with cache and
  failover providers.
- A location registry for edge nodes with TTL and heartbeat, exposed for routing decisions.
- Wire location into `help.getConfig`/nearest-DC hints.

## Capabilities

### New Capabilities

- `edge-location-registry`: client geolocation and edge location tracking.

## Impact

- **Crates**: `rtd-edge` (`location` module), possibly a small `geoip` crate.
- **Non-goals**: a full CDN/geo-routing policy.
