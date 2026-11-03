# Spec Delta

## Purpose

Defines peer resolving, caching and member management.

## ADDED Requirements

### Requirement: Peer resolution and caching

The client SHALL resolve users, chats and channels to entities carrying an access hash and SHALL
cache them, refreshing the cache from updates.

#### Scenario: Username resolves
- **WHEN** a caller resolves a username
- **THEN** the manager returns an entity with a usable `InputPeer`

#### Scenario: Update refreshes a peer
- **WHEN** an update changes a cached peer
- **THEN** the cached entity is replaced

### Requirement: Member management

The client SHALL provide channel/chat member iteration and right checks.

#### Scenario: Members are iterated
- **WHEN** a caller iterates channel members
- **THEN** pages of members are returned via the query iterators
