# Spec Delta

## Purpose

Extends inline/reply markup fidelity.

## ADDED Requirements

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
