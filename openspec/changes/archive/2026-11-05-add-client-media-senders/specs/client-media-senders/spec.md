# Spec Delta

## Purpose

Defines staged media senders for audio, video, GIF, dice, venues, games and web pages.

## ADDED Requirements

### Requirement: Staged media senders

The client SHALL build the media value for audio, video, GIF, dice, venue, game and web-page
sends and invoke `messages.sendMedia` with it and the caption.

#### Scenario: Document-backed media is uploaded
- **WHEN** audio, video or a GIF is sent
- **THEN** an `inputMediaUploadedDocument` carrying the matching document attributes is used

#### Scenario: Self-contained media is sent
- **WHEN** a dice, venue, game or web-page preview is sent
- **THEN** the matching `inputMedia*` value is used
