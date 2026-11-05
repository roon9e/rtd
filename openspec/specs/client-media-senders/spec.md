# client-media-senders Specification

## Purpose
Defines staged media senders for audio, video, GIF, dice, venues, games and web pages.

## Requirements

### Requirement: Staged media senders

The client SHALL build the media value for audio, video, GIF, dice, venue, game and web-page
sends and invoke `messages.sendMedia` with it and the caption.

#### Scenario: Document-backed media is uploaded
- **WHEN** audio, video or a GIF is sent
- **THEN** an `inputMediaUploadedDocument` carrying the matching document attributes is used

#### Scenario: Self-contained media is sent
- **WHEN** a dice, venue, game or web-page preview is sent
- **THEN** the matching `inputMedia*` value is used

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

### Requirement: Invoice sending

The client SHALL send an invoice built from a title, description, payload, provider data,
currency and price breakdown.

#### Scenario: Invoice media is built
- **WHEN** an invoice is sent
- **THEN** an `inputMediaInvoice` carrying the invoice is used

### Requirement: To-do sending

The client SHALL send a to-do list built from a title and items.

#### Scenario: To-do media is built
- **WHEN** a to-do list is sent
- **THEN** an `inputMediaTodo` carrying the list is used
