# Spec Delta

## Purpose

Defines how the client delivers server update messages to a caller-provided handler.

## ADDED Requirements

### Requirement: Update dispatch

The client SHALL offer every inbound server message that is not the answer to a request to a
caller-provided update handler, decoding it as an `Updates` envelope or, failing that, as a
bare `Update` wrapped in `Updates::updateShort`, and SHALL ignore a message that decodes as
neither.

#### Scenario: An update is delivered
- **WHEN** the server sends an update message while a call is in flight
- **THEN** the handler is invoked with the decoded `Updates` and the awaited result is still returned

#### Scenario: An unknown message is ignored
- **WHEN** a non-result message decodes as neither `Updates` nor `Update`
- **THEN** it is ignored and does not disturb the awaited result
