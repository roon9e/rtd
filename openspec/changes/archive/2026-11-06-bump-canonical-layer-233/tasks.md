# Tasks

## 1. Schema

- [x] 1.1 Import `layer-233.tl` and set `canonical_layer` 233 in the manifest and `rtd.json`
- [x] 1.2 Enable layers 225–233 and adopt `td`'s 305-entry `policy.json`

## 2. Generation

- [x] 2.1 Raise the generated-source line budget to fit the ninth profile
- [x] 2.2 Regenerate `rtd-tg`/`rtd-tlprofile`; update fixtures for the new `user` field
- [x] 2.3 Update docs and the `tl-codegen` spec to 233

## 3. Verification

- [x] 3.1 Confirm schema/policy/manifest parity with `td`
- [x] 3.2 `rtd-codegen verify` and `generate` report canonical 233
- [x] 3.3 Full workspace gate (`fmt`, `clippy -D warnings`, `test --locked`, generation freshness)
