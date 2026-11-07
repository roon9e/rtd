# Spec Delta

## ADDED Requirements

### Requirement: Family media projection

The system SHALL project a canonical `messageMediaWidget` to `messageMediaUnsupported` when
encoding it for an exact profile before 233, instead of dropping the media.

#### Scenario: Widget media projects to unsupported
- **WHEN** a canonical message media widget is encoded for a profile in 225–232
- **THEN** a `messageMediaUnsupported` is emitted

#### Scenario: Widget media survives at 233
- **WHEN** a canonical message media widget is encoded for profile 233
- **THEN** the widget is emitted unchanged
