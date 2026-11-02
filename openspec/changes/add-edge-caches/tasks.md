# Tasks

## 1. Caches

- [ ] 1.1 Make `AuthKeyCache` an LRU with `max_entries`/`ttl` (`rtd-server/src/session.rs`)
- [ ] 1.2 Add a temp-key cache for `bindTempAuthKey` bindings

## 2. Invalidation

- [ ] 2.1 Handle `auth_key_expiry`, `auth_key_switch` and `destroy_auth_key` invalidation and
  propagate to the store

## 3. Config

- [ ] 3.1 Add cache size/TTL flags/env to `rtd-edge` (`rtd-edge/src/config.rs`)

## 4. Verification

- [ ] 4.1 Verify eviction at `max_entries` and expiry at TTL (`rtd-server` tests)
- [ ] 4.2 Verify a destroyed key is not served from cache
- [ ] 4.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
