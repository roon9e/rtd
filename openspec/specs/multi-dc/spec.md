# Multi-DC Specification

## Purpose

Defines what it means for rtd to support a multi-datacenter server: advertising several
datacenters with their own identities and keys, routing clients and authorization keys to
the correct datacenter, directing clients to migrate, and delivering data across
datacenters.

## Requirements

### Requirement: Multiple datacenters are advertised
The server SHALL advertise the full set of configured datacenters, each with its own
identifier, address and RSA key, in the configuration returned to clients, rather than a
single local datacenter.

#### Scenario: Configuration lists every datacenter
- **WHEN** a client requests the server configuration
- **THEN** the response lists every configured datacenter with a distinct identifier, address and key

### Requirement: Per-datacenter keys and identities
Each datacenter SHALL have its own RSA key pair and key fingerprint, and the key exchange
SHALL use the key belonging to the datacenter the client is connecting to.

#### Scenario: Datacenter-specific key is used
- **WHEN** a client performs a key exchange with a given datacenter
- **THEN** that datacenter's key is used and its fingerprint is accepted

### Requirement: Datacenter-aware authorization keys and sessions
Authorization keys and sessions SHALL be associated with the datacenter that issued them,
and a request carrying an authorization key SHALL be routed to a datacenter that holds
that key.

#### Scenario: Unknown key on the wrong datacenter
- **WHEN** a request arrives at a datacenter that does not hold the client's authorization key
- **THEN** the client is directed to the correct datacenter or authorization is transferred as configured

### Requirement: Migration and redirect directives
The server SHALL be able to direct a client to another datacenter for phone, network,
user and file operations, exposing the target datacenter identifier in the error, and
media and file references SHALL point at the datacenter that stores them.

#### Scenario: Operation requires another datacenter
- **WHEN** an operation must run on a different datacenter
- **THEN** the server returns a migration error naming the target datacenter

#### Scenario: Media is served from its storing datacenter
- **WHEN** the server returns a media or file reference
- **THEN** the reference names the datacenter that actually stores it instead of always naming the local one

### Requirement: Authorization transfer between datacenters
The server SHALL support exporting and importing a user's authorization between
datacenters so a client can act on a non-primary datacenter.

#### Scenario: Authorization is transferred
- **WHEN** a client requests access to a datacenter other than its primary one
- **THEN** the server issues a transferable authorization and the target datacenter accepts it

### Requirement: Cross-datacenter delivery
The server SHALL deliver updates and messages to sessions regardless of which datacenter
holds them, without loss or duplication.

#### Scenario: Update reaches a session on another datacenter
- **GIVEN** two sessions of the same account connected to different datacenters
- **WHEN** an event occurs that both must receive
- **THEN** both receive it exactly once

### Requirement: Datacenter label validation policy
The server SHALL make datacenter-label validation during key exchange configurable,
allowing strict rejection of mismatched labels or permissive acceptance for diagnostics.

#### Scenario: Strict mode rejects mismatched label
- **WHEN** strict validation is enabled and a key exchange carries a datacenter label that does not match the serving datacenter
- **THEN** the exchange is rejected
