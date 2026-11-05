# client-tdp Specification

## Purpose
Defines structural, field-by-field pretty-printing of TL values.

## Requirements

### Requirement: Structural pretty-printing

The client SHALL render a TL object as its constructor name and named fields, recursively.

#### Scenario: Fields are named
- **WHEN** an object is pretty-printed
- **THEN** its constructor name and its field names appear in the output

#### Scenario: Compact and label forms
- **WHEN** the compact form is requested
- **THEN** the object is rendered on a single line, and the label is `name#id`
