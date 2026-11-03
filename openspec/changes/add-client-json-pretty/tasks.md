# Tasks

## 1. JSON

- [x] 1.1 Add a dependency-free `JsonValue` and constructor name lookup (`json.rs`)
- [ ] 1.2 Add `InvokeJSON` (decode method -> invoke -> encode result/error)

## 2. Pretty

- [ ] 2.1 Add `tdp::format` using the generated type descriptors

## 3. Verification

- [ ] 3.1 Verify a TL object round-trips through JSON (`json` tests)
- [ ] 3.2 Verify `InvokeJSON` against a mock server
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
