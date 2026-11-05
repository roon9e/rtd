# Spec Delta

## Purpose

Extends media-send options and adds story/paid-media senders.

## ADDED Requirements

### Requirement: Media send options

The client SHALL allow a media send to reply, be scheduled, be silent or background, disallow
forwarding and invert the media, through explicit options.

#### Scenario: Scheduled media send is carried
- **WHEN** a media send sets a schedule date
- **THEN** the built request carries that date

### Requirement: Story and paid-media senders

The client SHALL send a story by reference and send paid media.

#### Scenario: Story is sent
- **WHEN** a story is sent by reference
- **THEN** an `inputMediaStory` is used
