# Tasks

## 1. Result type

- [ ] 1.1 Add `InvokeResult` with `Object`/`Vector` variants
- [ ] 1.2 Recognize the `0x1cb5c415` vector magic in the result decoder
- [ ] 1.3 Add `Client::invoke_raw`; keep `invoke` returning `Object`

## 2. Verification

- [ ] 2.1 Verify a constructor result decodes to `InvokeResult::Object`
- [ ] 2.2 Verify a vector result decodes to `InvokeResult::Vector`
