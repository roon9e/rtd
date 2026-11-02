# Spec Delta

## Purpose

Defines client geolocation and edge location tracking.

## ADDED Requirements

### Requirement: Client location

The edge SHALL resolve a client IP to a coarse location through a cached provider chain and SHALL
degrade gracefully when providers fail.

#### Scenario: Location is cached
- **WHEN** the same IP is resolved twice within the cache TTL
- **THEN** the second resolution is served from the cache

#### Scenario: Provider failure degrades
- **WHEN** the primary provider is unavailable
- **THEN** the edge falls back to the next provider or an unknown location without failing the
  connection

### Requirement: Edge location registry

The edge SHALL register its location with a heartbeat and expire stale entries.

#### Scenario: Stale entry expires
- **WHEN** an edge stops heartbeating past the TTL
- **THEN** its registry entry is removed
