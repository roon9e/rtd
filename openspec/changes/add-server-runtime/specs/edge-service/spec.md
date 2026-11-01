# Spec Delta

## ADDED Requirements

### Requirement: Edge-initiated calls to the core
The edge SHALL call the core for admitted requests by dialing the core's execution
endpoint, sending the admitted-call metadata and canonical request bytes, and encoding the
returned canonical result for the session's layer.

#### Scenario: Admitted call reaches the core
- **WHEN** the edge admits a request and forwards it to the core
- **THEN** the core receives the session identity, semantic method, negotiated profile, wire identifier and canonical request, and returns a result

### Requirement: Interchangeable core implementations
The edge SHALL reach its core through one abstraction that both the gRPC adapter and an
in-process implementation satisfy, so the edge can run with either without changing
observable protocol behavior.

#### Scenario: In-process core serves a request
- **WHEN** the edge runs with an in-process core implementation
- **THEN** admitted requests are served with the same observable behavior as over the gRPC adapter

### Requirement: Core unavailable
When the core cannot be reached or fails, the edge SHALL return a TL error to the client
and SHALL keep the session alive.

#### Scenario: Core failure is surfaced
- **WHEN** the core is unreachable for an admitted request
- **THEN** the client receives a TL error and the connection remains usable
