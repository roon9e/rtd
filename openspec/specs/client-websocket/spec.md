# client-websocket Specification

## Purpose
Defines MTProto-over-WebSocket transport for the client.

## Requirements

### Requirement: WebSocket transport

The client SHALL connect to a `ws://`/`wss://` endpoint and frame MTProto messages using the
intermediate codec, presenting the connection as a byte-stream transport.

#### Scenario: WebSocket session completes the exchange
- **WHEN** the client dials a WebSocket endpoint and performs the auth-key exchange
- **THEN** it establishes an encrypted session

#### Scenario: Non-binary frame is rejected
- **WHEN** the peer sends a non-binary WebSocket frame
- **THEN** the transport reports an error
