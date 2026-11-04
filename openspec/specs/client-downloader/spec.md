# client-downloader Specification

## Purpose
Defines file download helpers with CDN support.

## Requirements

### Requirement: File download

The downloader SHALL fetch a file in ranges, verify hashes when requested, and support parallel
and streaming sinks.

#### Scenario: Verified download
- **WHEN** a file is downloaded with verification enabled
- **THEN** the assembled bytes match the server hashes

### Requirement: CDN redirection

When a file answers `upload.fileCdnRedirect`, the downloader SHALL fetch `upload.getCdnFile` and
verify `upload.getCdnFileHashes`, refreshing the token when it expires.

#### Scenario: CDN redirect is followed
- **WHEN** the master DC redirects a file to a CDN DC
- **THEN** the downloader fetches and verifies the file from the CDN
