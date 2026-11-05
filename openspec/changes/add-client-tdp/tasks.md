# Tasks

## 1. Metadata

- [ ] 1.1 Emit per-type `Field { name, kind }` metadata from `rtd-codegen`
- [ ] 1.2 Expose it from `rtd-tg`

## 2. Printer

- [ ] 2.1 Add a depth-bounded structural pretty-printer over the metadata
- [ ] 2.2 Render `[]byte` as base64 and integers/dates readably

## 3. Verification

- [ ] 3.1 Verify a nested object renders its field names and values
- [ ] 3.2 Verify generation freshness stays green
