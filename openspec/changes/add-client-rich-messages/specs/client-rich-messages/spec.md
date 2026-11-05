# Spec Delta

## Purpose

Defines construction of Telegram rich messages.

## ADDED Requirements

### Requirement: Rich message construction

The client SHALL build a rich message from page blocks containing rich text, and SHALL attach it
to a message send or edit.

#### Scenario: Rich message is attached
- **WHEN** a message is built with a rich message
- **THEN** the request carries the rich message

### Requirement: Rich parsing

The client SHALL parse a rich HTML/Markdown subset into page blocks.

#### Scenario: Headings and lists parse
- **WHEN** rich text containing a heading and a list is parsed
- **THEN** the corresponding page blocks are produced
