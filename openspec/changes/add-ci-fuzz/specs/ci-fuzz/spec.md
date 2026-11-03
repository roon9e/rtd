# Spec Delta

## Purpose

Defines fuzz targets and a bounded fuzz CI job.

## ADDED Requirements

### Requirement: Fuzz targets

The project SHALL provide fuzz targets for the key-exchange flow and for message, container,
gzip, TL-admission and RSA decoding, each asserting no panics and bounded allocation.

#### Scenario: Arbitrary input does not panic
- **WHEN** a target is fed arbitrary bytes
- **THEN** it returns an error rather than panicking

#### Scenario: Decompression bomb is bounded
- **WHEN** a target decodes a highly compressible payload
- **THEN** allocation stays within the configured cap
