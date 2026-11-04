# Spec Delta

## Purpose

Defines file-id, thumbnail and deep-link helpers.

## ADDED Requirements

### Requirement: File ids

The client SHALL encode and decode Bot-API/TDLib file ids and convert them to input file
locations.

#### Scenario: File id round-trips
- **WHEN** a file id is decoded and re-encoded
- **THEN** the bytes are identical to the original

### Requirement: Thumbnail expansion

The client SHALL reconstruct a displayable JPEG from a stripped thumbnail.

#### Scenario: Stripped thumbnail expands
- **WHEN** a stripped thumbnail is expanded
- **THEN** the result is a valid JPEG with the stripped bytes reinserted

### Requirement: Deep links

The client SHALL parse Telegram deep links into typed referrals.

#### Scenario: A resolve link parses
- **WHEN** a `t.me/<username>` link is parsed
- **THEN** the result is a `Resolve` referral
