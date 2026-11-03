# Spec Delta

## Purpose

Defines public advertisement in `help.getConfig`/`help.getNearestDc`.

## ADDED Requirements

### Requirement: Advertised endpoints

The edge SHALL advertise its configured public address, port and datacenter to clients.

#### Scenario: Clients learn the endpoint
- **WHEN** a client calls `help.getConfig`
- **THEN** the advertised address, port and datacenter are returned

### Requirement: Strict DC check

The edge SHALL honour the `strict_dc_check` setting when accepting the client's datacenter label.

#### Scenario: Strict check rejects a foreign DC
- **WHEN** `strict_dc_check` is enabled and the client sends a foreign datacenter
- **THEN** the exchange rejects it
