# Spec Delta

## Purpose

Adds a message-search iterator.

## ADDED Requirements

### Requirement: Message search

The client SHALL iterate the messages matching a query and filter in a peer, paging like the
history iterator.

#### Scenario: Search pages are returned
- **WHEN** a search iterator is advanced
- **THEN** each page of matching messages is returned until the search is exhausted
