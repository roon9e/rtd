# Spec Delta

## Purpose

Defines structural, field-by-field pretty-printing of TL values.

## ADDED Requirements

### Requirement: Structural pretty-printing

The client SHALL render a TL object as its constructor name and named fields, using generated
field metadata, with byte fields shown as base64.

#### Scenario: Fields are named
- **WHEN** an object is pretty-printed
- **THEN** each present field is shown with its name and rendered value

#### Scenario: Unknown type is tolerated
- **WHEN** a type has no field metadata
- **THEN** the printer falls back to `name#id`
