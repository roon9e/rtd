# Tasks

## 1. File ids

- [ ] 1.1 Add `FileID`/`Type` encode/decode with RLE (`fileid.rs`)
- [ ] 1.2 Add `FromDocument`/`FromPhoto` and `AsInputFileLocation`/`AsInputWebFileLocation`

## 2. Thumbnail and deeplink

- [ ] 2.1 Add `thumbnail::expand` and SVG handling
- [ ] 2.2 Add `deeplink::parse` with `Resolve`/`Join`/`BusinessChat` and domain validation

## 3. Verification

- [ ] 3.1 Verify file-id round-trips against `td` vectors (`fileid` tests)
- [ ] 3.2 Verify thumbnail expansion and deeplink parsing
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
