# client-rich-messages Specification

## Purpose
Defines construction of Telegram rich messages from blocks or raw HTML/Markdown.

## Requirements

### Requirement: Rich message construction

The client SHALL build a rich message from page blocks containing rich text, or from raw rich
HTML/Markdown, and SHALL attach it to a message send or edit.

#### Scenario: Rich message is attached
- **WHEN** a message or edit is built with a rich message
- **THEN** the request carries the rich message

### Requirement: Raw rich HTML and Markdown

The client SHALL encode raw rich HTML and Markdown as `inputRichMessageHTML` and
`inputRichMessageMarkdown`, leaving parsing to the server.

#### Scenario: HTML and Markdown map to their input variants
- **WHEN** a rich message is built from HTML or Markdown
- **THEN** the corresponding `inputRichMessageHTML` or `inputRichMessageMarkdown` is produced

### Requirement: Rich text nodes

The client SHALL provide builders for the common rich-text nodes (plain, bold, italic, fixed,
link, concatenation) and for paragraph and heading page blocks.

#### Scenario: Heading is built
- **WHEN** a rich message is built with a heading of level 1–6
- **THEN** the matching `pageBlockHeading<N>` block is produced
