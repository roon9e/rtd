# Spec Delta

## Purpose

Defines upload helpers for small and big files.

## ADDED Requirements

### Requirement: Small and big uploads

The uploader SHALL use `upload.saveFilePart` with an MD5 for files under the small limit and
`upload.saveBigFilePart` for larger files, and SHALL keep the part count within the limit.

#### Scenario: Large file upload
- **WHEN** a file larger than the small limit is uploaded
- **THEN** the uploader uses `saveBigFilePart` with a growing part size and stays within the
  part limit

### Requirement: Progress

The uploader SHALL report progress as bytes are uploaded and SHALL support cancellation.

#### Scenario: Progress is reported
- **WHEN** an upload proceeds
- **THEN** progress callbacks receive monotonically increasing uploaded bytes
