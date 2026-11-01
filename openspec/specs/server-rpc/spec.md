# Server RPC Specification

## Purpose

Defines how the server resolves and runs Telegram RPC requests: unwrapping invocation
wrappers, binding each session to a layer, encoding results for that layer, and mapping
failures to TL errors.

## Requirements

### Requirement: Invocation wrapper handling
The server SHALL unwrap invocation wrappers up to a declared maximum depth, capture
client and connection information from the initialization wrapper, and enforce any
ordering constraints implied by invoke-after wrappers. Requests exceeding the depth
limit or referencing too many ordering identifiers SHALL be rejected.

#### Scenario: Wrapper depth limit is enforced
- **WHEN** a request nests invocation wrappers beyond the declared maximum depth
- **THEN** the request is rejected with an error

#### Scenario: Client information is captured
- **WHEN** a request includes the initialization wrapper
- **THEN** the server records the supplied client information for the session

### Requirement: Layer binding per session
The server SHALL bind each session to a layer negotiated by explicit selection or
inherited from the authorization key, SHALL keep the binding ordered by client message
identifier when a client corrects its layer, and SHALL fail closed when the bound layer
is not supported.

#### Scenario: Inherited default applies
- **GIVEN** an authorization key with a recorded default layer
- **WHEN** a request on that key does not select a layer
- **THEN** the request is served using the inherited layer

#### Scenario: Correction is ordered
- **WHEN** a client corrects its negotiated layer
- **THEN** the correction takes effect in client message-identifier order and stale per-layer output is not delivered

#### Scenario: Unsupported bound layer fails closed
- **WHEN** a session's effective layer is not supported
- **THEN** the server refuses to serve the request and asks the client to reselect a layer

### Requirement: Result binding to the admitted call
The server SHALL encode a result only through the identity of the admitted call, and
SHALL reject a result whose identity does not match the admitted request.

#### Scenario: Mismatched result identity is rejected
- **WHEN** a handler returns a result whose identity differs from the admitted request
- **THEN** the server rejects the result instead of encoding it

### Requirement: Error mapping
The server SHALL map failures to TL errors with an integer code and a message, and
SHALL parse numeric suffixes out of error messages so that errors such as wait or
migration errors expose their argument.

#### Scenario: Wait error exposes its argument
- **WHEN** an error message ends with a numeric suffix
- **THEN** the mapped error reports both its type and the numeric argument

### Requirement: Unknown method handling
The server SHALL route an unknown method to a registered fallback when one exists, and
otherwise SHALL return a method-not-found error.

#### Scenario: Unknown method falls back
- **GIVEN** a registered fallback for unknown methods
- **WHEN** a request names a method the server does not implement
- **THEN** the fallback is invoked with the raw request

### Requirement: Handler isolation
The server SHALL isolate handler failures so that one failing request cannot terminate
the connection or the server.

#### Scenario: Panicking handler is contained
- **WHEN** a handler fails unexpectedly
- **THEN** the server returns an error for that request and keeps serving the connection
