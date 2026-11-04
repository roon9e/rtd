# client-entity-builder Specification

## Purpose
Defines programmatic composition of message text and its styled entities.

## Requirements

### Requirement: Styled text composition

The client SHALL build a message from plain and styled spans, producing the text and one or
more message entities whose offsets and lengths are measured in UTF-16 code units.

#### Scenario: Styled span gets a UTF-16 offset
- **WHEN** a styled span follows text containing an astral character
- **THEN** the entity offset counts the astral character as two UTF-16 code units

#### Scenario: Several styles on one span
- **WHEN** one span is given several styles
- **THEN** the text is appended once and one entity is produced per style over the same range
