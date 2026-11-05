# Spec Delta

## Purpose

Extends rich-message block coverage.

## ADDED Requirements

### Requirement: Rich block kinds

The client SHALL provide builders for paragraph and heading blocks plus lists, preformatted
blocks, block quotes, dividers, collapsible details, headers, subheaders, subtitles and footers.

#### Scenario: List block is built
- **WHEN** a list block is built from items
- **THEN** a `pageBlockList` carrying the items is produced

#### Scenario: Preformatted block is built
- **WHEN** a preformatted block is built with a language
- **THEN** a `pageBlockPreformatted` carrying the language is produced
