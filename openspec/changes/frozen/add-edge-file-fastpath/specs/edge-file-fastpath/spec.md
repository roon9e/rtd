# Spec Delta

## Purpose

Defines edge-resident file-data serving.

## ADDED Requirements

### Requirement: File methods are served at the edge

The edge SHALL serve file methods (`upload.getFile`, CDN, upload parts) from a configured file
service instead of the core, wrapping responses as normal MTProto results.

#### Scenario: A download bypasses the core
- **WHEN** a client calls `upload.getFile`
- **THEN** the edge answers from the file service and the core is not called

#### Scenario: File service failure falls back
- **WHEN** the file service errors
- **THEN** the edge forwards the call to the core
