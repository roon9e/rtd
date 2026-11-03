# Spec Delta

## Purpose

Defines bounded auth-key and temp-key caching with expiry/switch handling.

## ADDED Requirements

### Requirement: Bounded caching

The edge SHALL cache auth keys and temporary-key bindings within configured entry and TTL bounds.

#### Scenario: Cache evicts at capacity
- **WHEN** more keys than the maximum are cached
- **THEN** the least-recently-used entry is evicted

### Requirement: Invalidation

The edge SHALL invalidate a cached key on expiry, switch or destroy.

#### Scenario: Destroyed key is not served
- **WHEN** a key is destroyed
- **THEN** subsequent lookups do not return it from the cache
