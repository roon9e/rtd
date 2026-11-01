# Spec Delta

## Purpose

Turns versioned Telegram TL layer schemas plus a reviewed conversion policy into the
Rust canonical value model and the multi-layer routing sidecar, deterministically and
with verifiable upstream provenance.

## ADDED Requirements

### Requirement: Manifest-driven layer universe
The generator SHALL derive every layer it supports from a manifest that declares the
canonical layer and each supported layer's schema file. The set of supported layers
SHALL NOT require changes to generator source code.

#### Scenario: Adding a layer requires no code change
- **GIVEN** the generator accepts layers 225 through 232
- **WHEN** a new layer schema file and manifest entry are added and generation is run
- **THEN** the generator emits code for the new layer using the same paths as existing layers

#### Scenario: Unknown canonical layer is rejected
- **WHEN** the manifest declares a canonical layer that has no matching layer entry
- **THEN** generation fails with a diagnostic naming the missing layer

### Requirement: Schema provenance is recorded and verified
Each layer entry SHALL record upstream repository, commit, blob hash and content
SHA-256. Generation SHALL verify each schema file against its recorded SHA-256 and
SHALL fail if it does not match.

#### Scenario: Digest mismatch fails generation
- **WHEN** a schema file's bytes do not match the SHA-256 recorded in the manifest
- **THEN** generation fails and reports the expected and actual digests

#### Scenario: Bootstrap preserves tdesktop provenance
- **GIVEN** layer schemas originally fetched from the tdesktop repository
- **WHEN** they are imported into rtd
- **THEN** the manifest still records the original tdesktop repository, commit and blob for each schema

### Requirement: Explicit constructor identifiers
Every constructor and method line in an input schema SHALL carry an explicit wire
identifier. Import SHALL reject a schema containing a line without an explicit
identifier.

#### Scenario: Missing identifier is rejected
- **WHEN** a schema line omits its explicit wire identifier
- **THEN** import fails and identifies the offending line

### Requirement: Reviewed policy gates every conversion difference
The generator SHALL require a reviewed policy entry for every per-layer conversion
difference that is not a mechanical identifier or body reuse, and SHALL fail on
unreviewed differences and on policy entries that no longer match any difference.

#### Scenario: Unreviewed difference blocks generation
- **WHEN** two layers define the same semantic definition with a non-mechanical body difference and no policy entry exists
- **THEN** generation fails and lists the unresolved difference

#### Scenario: Stale policy entry blocks generation
- **WHEN** a policy entry no longer corresponds to any difference in the declared layers
- **THEN** generation fails and reports the entry as stale

### Requirement: Deterministic and atomic output
Given identical inputs, the generator SHALL produce byte-identical output. It SHALL
write generated files atomically so a failure cannot leave a partially updated
generation.

#### Scenario: Repeat generation is identical
- **WHEN** generation is run twice with identical inputs
- **THEN** every generated file is byte-for-byte identical between runs

#### Scenario: Failure leaves prior output intact
- **WHEN** generation fails partway through
- **THEN** no generated file is left in a partially written or mixed state

### Requirement: Generated source budget
The generator SHALL enforce a declared size and line budget for the generated
multi-layer sidecar and SHALL fail when the budget is exceeded.

#### Scenario: Budget breach fails generation
- **WHEN** generated multi-layer output exceeds the declared budget
- **THEN** generation fails and reports the measured size and the budget

### Requirement: Generated code is committed
Generation SHALL be performed by a standalone tool whose output is committed to the
repository, and SHALL NOT be triggered as part of a normal build.

#### Scenario: Normal build does not generate
- **WHEN** the workspace is built without explicitly running the generator
- **THEN** the build uses the committed generated sources and performs no schema generation
