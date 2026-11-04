# Spec Delta

## Purpose

Defines paginated query iterators.

## ADDED Requirements

### Requirement: Pagination iterators

The client SHALL provide iterators for participant, dialog, message, photo, blocked and sticker
queries that advance offsets automatically and terminate on a short or empty page.

#### Scenario: Dialogs are iterated
- **WHEN** a caller iterates dialogs
- **THEN** pages are fetched until a short page is returned

### Requirement: Cached queries

Cached queries SHALL fetch current hashes and request only changed items.

#### Scenario: Only changed items are fetched
- **WHEN** a cached query's items are unchanged
- **THEN** no full item fetch is issued
