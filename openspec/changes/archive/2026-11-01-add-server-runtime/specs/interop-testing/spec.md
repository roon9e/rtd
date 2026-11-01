# Spec Delta

## ADDED Requirements

### Requirement: Full-session external-peer gate
The external-peer gate SHALL establish a complete encrypted session against the official
engine's test server — the authorization-key exchange followed by encrypted service
traffic — and SHALL require a `pong` response to a `ping`, in addition to the handshake.

#### Scenario: Encrypted ping is answered
- **WHEN** rtd completes a key exchange with the official test server and sends an encrypted ping
- **THEN** the server answers with a matching pong and the gate passes

### Requirement: In-process end-to-end session
The project SHALL include an end-to-end test in which a test client completes a key
exchange and an RPC against an rtd edge backed by a fake core, over each supported ingress
transport.

#### Scenario: Session over a plain TCP framing
- **WHEN** a test client connects over a supported TCP framing, exchanges a key and calls a built-in method
- **THEN** the exchange and the call succeed over the wire

#### Scenario: Session over an MTProxy transport
- **WHEN** a test client connects through MTProxy obfuscation, exchanges a key and calls a built-in method
- **THEN** the exchange and the call succeed over the wire
