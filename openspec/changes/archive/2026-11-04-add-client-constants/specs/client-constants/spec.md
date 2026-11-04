# Spec Delta

## Purpose

Defines Telegram-defined constants and TDLib peer-id encoding for the client.

## ADDED Requirements

### Requirement: TDLib peer ids

The client SHALL convert between TDLib peer ids and plain MTProto ids for users, chats and
channels, and SHALL classify an id as exactly one of user, chat or channel, with monoforums
recognized as channels.

#### Scenario: Plain conversion round-trips
- **WHEN** a TDLib user, chat or channel id is converted to its plain id and back with the matching constructor
- **THEN** the original TDLib id is recovered

#### Scenario: Classification is exclusive
- **WHEN** an id is classified
- **THEN** it is a user, a chat or a channel according to the TDLib ranges, and a monoforum id is also a channel

### Requirement: Telegram constants

The client SHALL expose the well-known Telegram user IDs, upload limits and test credentials as
named constants.

#### Scenario: Upload limits are available
- **WHEN** a caller chooses between `upload.saveFilePart` and `upload.saveBigFilePart`
- **THEN** the named small-file size and part limits are available as constants
