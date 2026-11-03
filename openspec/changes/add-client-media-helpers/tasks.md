# Tasks

## 1. File ids

- [x] 1.1 Add `FileId`/`Type`/`PhotoSizeSource` encode/decode with RLE (`fileid.rs`)
- [ ] 1.2 Add `FromDocument`/`FromPhoto` and `AsInputFileLocation`/`AsInputWebFileLocation`

## 2. Thumbnail and deeplink

- [x] 2.1 Add `thumbnail::expand` (header derived from the reference; SVG pending)
- [x] 2.2 Add `deeplink::parse` with `Resolve`/`Join` and domain validation

## 3. Verification

- [x] 3.1 Verify file-id round-trips against `td` vectors (`fileid` tests)
- [x] 3.2 Verify thumbnail expansion and deeplink parsing
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
