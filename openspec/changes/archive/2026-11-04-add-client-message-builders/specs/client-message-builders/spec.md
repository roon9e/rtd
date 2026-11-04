# Spec Delta

## Purpose

Defines typed message, media and markup construction and sending.

## ADDED Requirements

### Requirement: Message construction

The client SHALL provide builders that construct `messages.sendMessage`/`sendMedia` requests with
options (silent, background, reply, schedule, markup) and content senders for common media.

#### Scenario: A formatted message is sent
- **WHEN** a caller sends HTML text through the sender
- **THEN** the resulting request carries the text and its parsed entities

### Requirement: Markup and inline results

The client SHALL provide inline-keyboard and inline-result builders that produce the correct TL
objects.

#### Scenario: A keyboard is attached
- **WHEN** a caller attaches a keyboard to a message
- **THEN** the request carries the corresponding `ReplyMarkup`
