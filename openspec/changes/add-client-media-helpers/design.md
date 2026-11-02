# Design

## Context

File ids are a packed binary format (type, id, access hash, DC, file reference) with RLE for
repeated bytes. Stripped thumbnails are a stripped JPEG whose bytes must be reinserted at the
`ffd9` marker. Deep links are URL forms of Telegram referrals.

## Goals / Non-Goals

**Goals**

- Round-trip file ids and convert to input locations.
- Correct thumbnail reconstruction.

**Non-Goals**

- Rendering; web page scraping.

## Decisions

### D1. `fileid` mirrors `td`'s `Type` enum

Encode/decode with the type-specific field layout and RLE, and provide
`FromDocument`/`FromPhoto` and `AsInputFileLocation`/`AsInputWebFileLocation`.

### D2. `thumbnail::Expand` reinserts bytes

Reconstruct the JPEG by inserting the stripped bytes before the final `ffd9`, as `td` does.

### D3. `deeplink` returns a typed enum

`Parse(url) -> Deeplink` yields `Resolve`/`Join`/`BusinessChat` with validation of the host in
`ValidateDomain`.

## Risks / Trade-offs

- [Format drift] → pin against `td`'s test vectors.
