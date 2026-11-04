# Tasks

## 1. JSON

- [x] 1.1 Add a dependency-free `JsonValue` and constructor name lookup (`json.rs`)
- [x] 1.2 Add `InvokeJSON` (decode method -> invoke -> encode result/error)

## 2. Pretty

- [x] 2.1 Add `tdp::format` using the generated type descriptors (`json::format`; the generated model exposes constructor names, not per-field descriptors, so the payload renders as its encoded form)

## 3. Verification

- [x] 3.1 Verify a TL object round-trips through JSON (`json` tests)
- [x] 3.2 Verify `InvokeJSON` against a mock server
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
