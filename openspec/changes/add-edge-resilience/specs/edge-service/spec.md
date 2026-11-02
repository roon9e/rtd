# Spec Delta

## ADDED Requirements

### Requirement: The listener survives a bad connection

The edge SHALL keep accepting connections after a connection fails transport detection or an
accept is interrupted; a single bad or probed connection SHALL NOT stop the listener.

#### Scenario: A probe does not stop the edge
- **WHEN** a client connects and closes before its transport header arrives
- **THEN** the edge logs or ignores the failure and continues accepting new connections

### Requirement: Startup does not depend on core readiness

The edge SHALL start and listen even when the core is not reachable at startup, and SHALL serve
requests once the core becomes reachable, rather than exiting and restart-looping.

#### Scenario: The edge starts before the core is ready
- **WHEN** the edge is started while the core is unavailable
- **THEN** it still binds and listens, and requests succeed once the core is reachable
