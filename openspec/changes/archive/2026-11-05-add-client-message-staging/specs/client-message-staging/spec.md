# Spec Delta

## Purpose

Defines message staging: scheduling, replies, typing, drafts, reports, joins and stickers.

## ADDED Requirements

### Requirement: Scheduled and reply sends

The client SHALL allow a message or media send to be scheduled for a date and to reply to a
specific message.

#### Scenario: Scheduled send is carried
- **WHEN** a send sets a schedule date
- **THEN** the built request carries that date

### Requirement: Staging helpers

The client SHALL provide helpers to send a sticker, set the typing action, save a draft, report
messages and join a channel.

#### Scenario: Typing action is sent
- **WHEN** the client sets a typing action for a peer
- **THEN** `messages.setTyping` is invoked with that action
