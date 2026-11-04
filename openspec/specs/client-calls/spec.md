# client-calls Specification

## Purpose
Defines one-to-one and group VoIP call signaling, encryption, and media.

## Requirements

### Requirement: One-to-one calls

The client SHALL place and answer 1:1 calls, performing the DH exchange, encrypting control
messages, and exchanging audio over a WebRTC connection.

#### Scenario: A call is established
- **WHEN** a caller places a call and the callee accepts
- **THEN** the call connects and audio flows in both directions

### Requirement: Group calls

The client SHALL join and leave group calls and stream audio to the SFU.

#### Scenario: A group call is joined
- **WHEN** the client joins a group call
- **THEN** it sends audio and receives participant updates
