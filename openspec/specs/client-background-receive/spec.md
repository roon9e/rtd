# client-background-receive Specification

## Purpose
Defines a client receive path that drains unsolicited server messages independently of any
in-flight call.

## Requirements

### Requirement: Independent receive path

The client SHALL read inbound frames independently of an awaiting call, and SHALL deliver
unsolicited updates, RPC results and errors to the caller as events.

#### Scenario: Update arrives between calls
- **WHEN** the server sends an update while no call is pending
- **THEN** the client delivers it to the update handler without requiring a subsequent call

#### Scenario: Peer closes
- **WHEN** the transport peer is dropped
- **THEN** the client reports a single `Closed` event and the reader stops

#### Scenario: Split preserves framing
- **WHEN** the transport is split into a reader and a writer
- **THEN** both directions use the same codec and framing as the unsplit transport
