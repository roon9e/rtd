# Proposal

## Why

Sending media requires `upload.saveFilePart`/`saveBigFilePart` with MD5, part sizing, threads
and progress. `rtd-client` has none of this. `td` ships `telegram/uploader` with small/big
uploads, auto part-size growth and progress.

## What Changes

- An `Uploader` supporting small (`< 10 MB`, `saveFilePart`, MD5) and big files
  (`saveBigFilePart`), part sizing, multi-threaded parts and progress reporting.
- Auto part-size growth for large files to stay within the 3999-part limit.
- A `Source` abstraction (bytes, reader, HTTP URL).

## Capabilities

### New Capabilities

- `client-uploader`: upload helpers for small and big files.

## Impact

- **Crates**: `rtd-client` (new `uploader` module); depends on `add-client-pool`.
- **Non-goals**: message sending (message builders).
