# Spec Delta

## ADDED Requirements

### Requirement: Non-strict label acceptance is scoped to a single backend

Permissive datacenter-label acceptance SHALL apply only to a single-backend deployment. A
multi-datacenter runtime SHALL treat the client's datacenter label as routing identity: it SHALL
select the backend holding that datacenter (or refuse an unknown label) and SHALL keep the label
part of the authorization-key and session identity, rather than serving the local datacenter for
every label.

#### Scenario: A client uses a high datacenter label

- **GIVEN** a multi-server client that multiplexes many accounts and selects a datacenter label above the locally served datacenters
- **WHEN** it performs the key exchange and issues requests
- **THEN** the runtime routes it to the backend for that label (or returns an explicit error), and does not silently serve it from the local datacenter
