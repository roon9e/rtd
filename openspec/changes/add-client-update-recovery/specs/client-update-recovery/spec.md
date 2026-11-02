# Spec Delta

## Purpose

Defines ordered, gap-aware update processing and recovery for the client.

## ADDED Requirements

### Requirement: Ordered application

The client SHALL apply updates in `pts`/`qts`/`seq` order, buffering updates that are ahead and
requesting a difference when a sequence is behind.

#### Scenario: Out-of-order update is buffered
- **WHEN** an update arrives ahead of the expected sequence
- **THEN** it is buffered until the gap is filled

#### Scenario: Gap triggers a difference
- **WHEN** the expected update does not arrive within the gap timeout
- **THEN** the client requests `updates.getDifference` and applies the result

### Requirement: Channel recovery

The client SHALL track per-channel state and request `updates.getChannelDifference` when a
channel sequence gaps, bounded by a concurrency cap.

#### Scenario: Channel gap is recovered
- **WHEN** a channel update is ahead of its channel sequence
- **THEN** the client requests the channel difference and applies the updates in order
