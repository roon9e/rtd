# Design

## Context

Telegram splits uploads at 10 MB: below that `upload.saveFilePart` with an MD5 of the whole
file; above, `upload.saveBigFilePart` with a random `file_id`, up to 3999 parts. `td` grows the
part size with the file size to stay within the limit (#816).

## Goals / Non-Goals

**Goals**

- Correct small/big selection and part accounting.
- Progress and cancellation.

**Non-Goals**

- Downloading; message builders.

## Decisions

### D1. Part size grows with file size

`part_size(file_size)` starts at 512 KiB and grows so `parts <= 3999`, 4 KiB-divisible,
matching `td`. An explicit `WithPartSize` overrides.

### D2. Threads for big uploads

Big files upload parts concurrently through the pool with a bounded worker count; small files
are a single call.

### D3. Progress is a trait

`Progress` receives `(uploaded, total)` updates; a no-op default keeps call sites clean.

## Risks / Trade-offs

- [MD5 over a stream] → buffer or require `Read + Seek` for small files; document the bound.
