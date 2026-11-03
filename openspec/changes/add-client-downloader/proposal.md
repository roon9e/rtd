# Proposal

## Why

Downloading files requires `upload.getFile`/`getWebFile`/CDN redirection, range assembly, hash
verification and concurrency. `rtd-client` has none of this. `td` ships
`telegram/downloader` with CDN support, parallel streams and verification.

## What Changes

- A `Downloader` with part sizing, `WithThreads`, `WithVerify`, streaming and
  `io::WriteAt`-style parallel writes.
- Schema implementations: master (precise ranges), web, parallel, and CDN (master -> CDN
  redirect state machine with token refresh and inline hash verification).
- Retry handling for wrapped transport timeouts.

## Capabilities

### New Capabilities

- `client-downloader`: file download helpers with CDN support.

## Impact

- **Crates**: `rtd-client` (new `downloader` module); depends on `add-client-pool` and
  `add-client-dcs` for CDN/DC targets.
- **Non-goals**: the file service itself (an edge change).
