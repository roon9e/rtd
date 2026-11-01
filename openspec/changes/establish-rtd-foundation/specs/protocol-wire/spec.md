# Spec Delta

## Purpose

Defines the byte-level TL serialization and MTProto 2.0 message framing that every
other part of the system builds on, including how messages are identified, grouped,
optionally compressed, and carried by each transport codec.

## ADDED Requirements

### Requirement: TL primitive serialization
The system SHALL serialize and deserialize TL primitives using little-endian integers
and the TL rules for booleans, strings, byte strings and vectors, including the long
string form for lengths of 254 bytes or more and the required zero padding to a 4-byte
boundary.

#### Scenario: Round-trip of primitive values
- **WHEN** a value containing integers, booleans, strings, byte strings and a vector is encoded and then decoded
- **THEN** the decoded value equals the original

#### Scenario: Long string form is used above the threshold
- **WHEN** a string of 254 bytes or more is encoded
- **THEN** the long length encoding is used and the payload remains 4-byte aligned

#### Scenario: Malformed vector count is rejected
- **WHEN** a vector declares a negative element count
- **THEN** decoding fails with an error

### Requirement: MTProto message identifiers
The system SHALL generate and interpret MTProto message identifiers encoding the send
time with a type in the low bits, and SHALL provide a replay window that rejects
identifiers that are duplicates or outside the acceptable age range.

#### Scenario: Message id carries time and type
- **WHEN** a message identifier is generated for a given time and message type
- **THEN** the send time and type can be recovered from the identifier

#### Scenario: Replayed identifier is rejected
- **WHEN** an identifier already seen within the replay window is presented again
- **THEN** the replay window rejects it

### Requirement: Message containers and nested messages
The system SHALL encode and decode MTProto message containers whose elements carry a
message identifier, sequence number, length and body, and SHALL reject an element
whose declared length is negative or exceeds the per-message bound.

#### Scenario: Oversized container element is rejected
- **WHEN** a container element declares a body length beyond the per-message bound
- **THEN** decoding fails with an error

### Requirement: Bounded decompression
The system SHALL decompress packed payloads only up to a declared maximum expanded
size and SHALL reject a payload whose expansion would exceed that bound.

#### Scenario: Decompression bomb is rejected
- **WHEN** a packed payload would expand beyond the declared maximum
- **THEN** decoding fails with an error identifying the expansion

### Requirement: RPC result framing
The system SHALL encode and decode RPC results that bind a request message identifier
to a result payload, and SHALL allow a result payload that is itself compressed.

#### Scenario: Result maps back to its request
- **WHEN** an RPC result carrying a request message identifier is decoded
- **THEN** the request message identifier and result payload are reported

### Requirement: Transport codecs
The system SHALL implement the abridged, intermediate, padded-intermediate and full
transport framings with their exact headers, length encodings and padding rules, and
SHALL detect the negotiated framing on an inbound connection.

#### Scenario: Framing is auto-detected
- **WHEN** a connection begins with one of the supported framing headers
- **THEN** the corresponding codec is selected

#### Scenario: Full framing checks are enforced
- **WHEN** a full-framing frame carries a mismatched sequence number or checksum
- **THEN** decoding fails with an error

#### Scenario: Declared length is read in bounded chunks
- **WHEN** a frame declares a large length
- **THEN** the system reads the payload progressively instead of pre-allocating the full declared size

### Requirement: Transport protocol errors
The system SHALL interpret a single trailing word in a frame as a transport protocol
error code, including key-not-found, wrong-DC and flood codes.

#### Scenario: Transport error code is surfaced
- **WHEN** a peer reports a transport protocol error code
- **THEN** the system surfaces that code to its caller

### Requirement: Obfuscated and proxied transports
The system SHALL support obfuscated2 and FakeTLS framing used by MTProxy, including
secret-based key derivation and DC labelling carried in the obfuscation header.

#### Scenario: Obfuscated connection is unwrapped
- **WHEN** an inbound connection uses obfuscated2 framing with a valid secret
- **THEN** the system unwraps it and reports the negotiated transport and DC label
