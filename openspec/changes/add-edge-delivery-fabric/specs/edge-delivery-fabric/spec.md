# Spec Delta

## Purpose

Defines cross-DC result/update delivery with replay and retention.

## ADDED Requirements

### Requirement: Cross-DC delivery

The edge SHALL deliver RPC results and updates to the datacenter where the client is connected,
through a delivery service, and SHALL retain them until the client acknowledges.

#### Scenario: Delivery is replayed after reconnect
- **WHEN** a client reconnects before acknowledging a delivery
- **THEN** the unacknowledged delivery is replayed

#### Scenario: Retention is bounded
- **WHEN** a delivery is acknowledged
- **THEN** it is pruned and retention stays within the configured bounds
