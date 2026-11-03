# Design

## Context

The edge reads auth keys through `Core`; a per-edge cache avoids a store round-trip per message.
gramsrv bounds the cache and handles temporary (`auth.bindTempAuthKey`) keys and keys that are
destroyed or re-created.

## Goals / Non-Goals

**Goals**

- Bounded memory with correct invalidation.
- Fast path for temporary keys.

**Non-Goals**

- Cross-edge cache coherence (handled by the store).

## Decisions

### D1. LRU with TTL

`AuthKeyCache` becomes an LRU keyed by `auth_key_id` with `max_entries` and `ttl`; eviction is
cheap and never changes protocol behavior. `TempKeyCache` caches `bindTempAuthKey` bindings with
their own TTL.

### D2. Expiry and switch invalidate

`auth_key_expiry`, `auth_key_switch` and `destroy_auth_key` update or evict the cached entry and
propagate to the store, matching gramsrv's `destroy_auth_key.go`.

### D3. Config parity

`rtd-edge` exposes `--auth-key-cache-max/ttl` and `--temp-key-cache-max/ttl`, mapping to
gramsrv's `edge.auth_key_cache`/`temp_key_cache`.

## Risks / Trade-offs

- [TTL too long hides key rotation] → short default TTL (30m) plus explicit invalidation on
  switch/destroy.
