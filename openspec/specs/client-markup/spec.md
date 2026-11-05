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

### Requirement: Inline button kinds

The client SHALL build inline buttons for callback, URL, web view, copy, switch-inline, user
profile, buy, game and disabled actions.

#### Scenario: Rich inline button is built
- **WHEN** a web-view or copy inline button is built
- **THEN** the matching `inlineButtonType*` value is produced

### Requirement: Bot menu buttons

The client SHALL build the commands, default and URL bot-menu buttons.

#### Scenario: Commands menu is built
- **WHEN** the commands bot menu is requested
- **THEN** a `botMenuButtonCommands` is produced

### Requirement: Request-peer reply button

The client SHALL build a reply-keyboard button that requests a peer.

#### Scenario: Request-peer button is built
- **WHEN** a reply button requests a peer
- **THEN** a `buttonTypeRequestPeer` carrying the peer type is produced
