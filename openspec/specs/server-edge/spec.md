# Server Edge Specification

## Purpose

Defines the server-side MTProto edge: accepting connections, establishing and
identifying a session's authorization key, enforcing admission budgets, validating
message identifiers and sequence numbers, and handling protocol-level service messages.

## Requirements

### Requirement: Inbound transport detection
The edge SHALL detect the transport framing of each inbound connection, including the
plaintext framing tags, obfuscated2, and WebSocket, and SHALL report the detected codec
and any DC label.

#### Scenario: Plaintext and obfuscated connections are both accepted
- **WHEN** two connections arrive, one plaintext and one obfuscated
- **THEN** each is served using its own detected framing

### Requirement: Authorization key exchange
The edge SHALL perform the server side of the authorization key exchange, supporting
both permanent and temporary keys, and SHALL persist a newly created authorization key
before confirming completion to the client.

#### Scenario: Temporary key request is accepted
- **WHEN** a client requests a temporary authorization key
- **THEN** the edge completes the exchange and records the key with its expiry

#### Scenario: Key is durable before confirmation
- **WHEN** the exchange completes
- **THEN** the authorization key has been stored before the success response is written

#### Scenario: Already-encrypted first frame is not rejected as unknown key
- **WHEN** the first frame is an encrypted message from a peer that already holds a valid key
- **THEN** the edge processes it rather than replying that the key is unknown

### Requirement: Session lifecycle
The edge SHALL maintain session state keyed by authorization key and session identifier,
SHALL deliver a session-created notification once per session, and SHALL release a
session's resources on disconnect.

#### Scenario: Session-created is sent once
- **WHEN** a valid encrypted message arrives on a newly created session
- **THEN** the edge sends the session-created notification once for that session

### Requirement: Message validation and replay protection
The edge SHALL validate inbound message identifiers and sequence numbers, reject
messages outside the acceptable time window, and detect replays, while tolerating
duplicate retransmissions.

#### Scenario: Out-of-window message is rejected
- **WHEN** an inbound message identifier is outside the accepted time window
- **THEN** the edge rejects it without processing its payload

### Requirement: Service message handling
The edge SHALL handle protocol service messages including ping and delayed ping,
message acknowledgements, future-salt requests, resend requests, bad-message and
bad-salt notifications, and session destruction.

#### Scenario: Ack request is answered
- **WHEN** a client requests message acknowledgements
- **THEN** the edge returns acknowledgements for the requested message identifiers

### Requirement: Outbound message flow
The edge SHALL assign message identifiers and sequence numbers to outbound messages
under a single writer per connection, and SHALL track outbound content messages so that
unacknowledged messages can be resent.

#### Scenario: Sequence numbers follow the content rule
- **WHEN** the edge sends a mix of content and service messages
- **THEN** each message's sequence number follows the MTProto content-message rule

### Requirement: Admission budget
The edge SHALL enforce configurable limits on total connections, connections per source
address, concurrent handshakes, and aggregate frame bytes, and SHALL refuse traffic that
exceeds a limit without destabilizing admitted sessions.

#### Scenario: Connection limit is enforced
- **WHEN** new connections exceed the configured total or per-source limit
- **THEN** the edge refuses them while admitted sessions continue normally
