# client-query-extras Specification

## Purpose
Defines featured-sticker and cached query helpers.

## Requirements

### Requirement: Featured stickers

The client SHALL iterate the account's featured stickers.

#### Scenario: Featured stickers are listed
- **WHEN** the featured-sticker query is run
- **THEN** each featured sticker set is yielded

### Requirement: Cached queries

The client SHALL cache a query result by its server hash and return the cached value when the
hash is unchanged.

#### Scenario: Cache hit skips the call
- **WHEN** a cached query is repeated with an unchanged hash
- **THEN** the stored value is returned without another request

### Requirement: Message search

The client SHALL iterate the messages matching a query and filter in a peer, paging like the
history iterator.

#### Scenario: Search pages are returned
- **WHEN** a search iterator is advanced
- **THEN** each page of matching messages is returned until the search is exhausted
