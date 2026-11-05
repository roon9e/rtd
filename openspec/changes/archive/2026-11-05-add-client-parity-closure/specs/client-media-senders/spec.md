# Spec Delta

## Purpose

Adds invoice and to-do senders.

## ADDED Requirements

### Requirement: Invoice sending

The client SHALL send an invoice built from a title, description, payload, provider data,
currency and price breakdown.

#### Scenario: Invoice media is built
- **WHEN** an invoice is sent
- **THEN** an `inputMediaInvoice` carrying the invoice is used

### Requirement: To-do sending

The client SHALL send a to-do list built from a title and items.

#### Scenario: To-do media is built
- **WHEN** a to-do list is sent
- **THEN** an `inputMediaTodo` carrying the list is used
