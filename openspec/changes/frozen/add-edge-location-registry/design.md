# Design

## Context

gramsrv resolves client IP -> location with a provider chain (MaxMind-style DB, HTTP service)
behind a cache and circuit breaker, and registers edge locations with a TTL and heartbeat.

## Goals / Non-Goals

**Goals**

- Accurate-enough client location with caching and degradation.
- A live view of edge locations.

**Non-Goals**

- Precise geolocation; a CDN.

## Decisions

### D1. A provider chain with cache and breaker

`LocationProvider` implementations (local DB, remote service) are tried in order behind a TTL
cache and a circuit breaker, mirroring `internal/geoip`.

### D2. Location registry keyed by instance id

Each edge registers `(instance_id, location, advertise, expiry)` and heartbeats; entries expire
by TTL. This pairs with `add-edge-identity-metrics` for identity.

### D3. Location feeds config hints

`help.getConfig` may include DC options chosen by location; the registry exposes the current
view to the config builder.

## Risks / Trade-offs

- [Privacy] → location is used only for routing; never logged per-peer beyond coarse country.
