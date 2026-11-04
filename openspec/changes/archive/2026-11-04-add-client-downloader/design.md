# Design

## Context

`upload.getFile` returns a range with a `FileHash` when the offset is 0 (and on request); CDN
files answer `upload.fileCdnRedirect` pointing at a CDN DC, where `upload.getCdnFile` and
`upload.getCdnFileHashes` are used. `td` models this as a state machine with a verified window.

## Goals / Non-Goals

**Goals**

- Correct range assembly, verification and CDN redirection.
- Parallel downloads with retry on transient timeouts.

**Non-Goals**

- Uploading; the file service.

## Decisions

### D1. Pluggable schemas over one sink

`Builder` targets a sink (`Stream`, `Parallel` via `WriteAt`, `ToPath`); a `Schema` decides the
next range. `Weiter` (master) if a normal file; `Web` for `inputWebFileLocation`; `CDN` wraps
the master with the redirect state machine.

### D2. Verified window cache for CDN

Keep a bounded map of verified byte ranges so re-requested CDN parts are not re-hashed,
mirroring `td`'s `cdn.go`/`cdn_verify.go`.

### D3. Retry transport timeouts

A wrapped transport timeout (#1725) is retried with the same range rather than failing the
whole download.

## Risks / Trade-offs

- [CDN token expiry] → refresh via `upload.reuploadCdnFile` before retrying.
