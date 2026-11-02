# Spec Delta

## Purpose

Defines ordered update projection with per-channel watermarks.

## ADDED Requirements

### Requirement: Ordered projection

The edge SHALL project updates to subscribed clients in order and SHALL encode them at the
client's negotiated layer.

#### Scenario: Updates are ordered
- **WHEN** several updates are projected to a client
- **THEN** they are delivered in pts/qts/seq order

### Requirement: Watermark resume

The edge SHALL track a per-client, per-channel watermark and SHALL resume delivery from it after
a reconnect.

#### Scenario: Reconnect resumes delivery
- **WHEN** a client reconnects after acknowledging updates up to a pts
- **THEN** the edge delivers updates after that pts
