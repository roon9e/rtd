# Proposal

## Why

`rtd-edge` keeps a transient `SessionRegistry` and an `AuthKeyCache`, but with no size or TTL
tuning, no temp-key cache, and no expiry/switch handling. gramsrv's edge configures
`auth_key_cache`/`temp_key_cache` (`max_entries`, `ttl`) and handles key expiry/switch
(`auth_key_expiry`, `auth_key_switch`, `destroy_auth_key`).

## What Changes

- Size- and TTL-bounded auth-key and temp-key caches with eviction.
- Auth-key expiry and switch handling (`auth_key_expiry`, `auth_key_switch`), including
  `destroy_auth_key`.
- Cache metrics and warm-up from the durable store (`add-server-storage`).

## Capabilities

### New Capabilities

- `edge-caches`: bounded auth-key and temp-key caching with expiry/switch handling.

## Impact

- **Crates**: `rtd-server` (`session.rs`), `rtd-edge` (config).
- **Non-goals**: the durable store itself.
