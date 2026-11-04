# client-markup Specification

## Purpose
Defines inline and reply keyboard builders for outgoing messages.

## Requirements

### Requirement: Inline keyboards

The client SHALL build a `replyInlineMarkup` from rows of callback and URL buttons.

#### Scenario: Inline rows are built
- **WHEN** rows of callback and URL buttons are supplied
- **THEN** the result is a `replyInlineMarkup` with one inner row per supplied row

### Requirement: Reply keyboards

The client SHALL build a `replyKeyboardMarkup` from rows of reply buttons, carrying the resize,
single-use, selective, persistent and force-reply options plus an optional placeholder.

#### Scenario: Reply keyboard carries buttons and options
- **WHEN** rows of reply buttons and options are supplied
- **THEN** the result is a `replyKeyboardMarkup` whose rows and option flags match the input

### Requirement: Keyboard control markups

The client SHALL build a `replyKeyboardHide` and a `replyKeyboardForceReply`.

#### Scenario: Hide and force-reply are built
- **WHEN** a caller hides the keyboard or requests a forced reply
- **THEN** the matching `replyKeyboardHide` or `replyKeyboardForceReply` is produced
