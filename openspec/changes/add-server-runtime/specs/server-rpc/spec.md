# Spec Delta

## ADDED Requirements

### Requirement: Configurable method handlers
The set of RPC methods the server implements SHALL be supplied by a router an embedder can
extend, so new methods can be added without changing the dispatch machinery, and a method
the router does not handle SHALL follow the unknown-method behavior.

#### Scenario: Extended router handles a method
- **WHEN** an embedder registers a handler for a method and a request for it is admitted
- **THEN** the registered handler runs and its result is returned

#### Scenario: Unhandled method falls through
- **WHEN** a router does not implement a requested method
- **THEN** the unknown-method behavior applies

### Requirement: Built-in configuration methods
The server SHALL implement at least `help.getConfig` and `help.getNearestDc`, returning
the configured datacenter set and identifying the datacenter serving the request.

#### Scenario: Configuration is returned
- **WHEN** a client calls `help.getConfig`
- **THEN** the response lists the configured datacenters

#### Scenario: Nearest datacenter is reported
- **WHEN** a client calls `help.getNearestDc`
- **THEN** the response identifies the datacenter serving the request
