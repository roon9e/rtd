# Tasks

## 1. Client

- [ ] 1.1 Add `Client::self_user` (`users.getUsers` with `inputUserSelf`)
- [ ] 1.2 Add JSON text parsing/serialization to `json`
- [ ] 1.3 Add `Client::invoke_raw_json` over the JSON codec

## 2. Helpers

- [ ] 2.1 Add ASCII rune predicates

## 3. Verification

- [ ] 3.1 Verify `self_user` returns the self user against a mock server
- [ ] 3.2 Verify a JSON string round-trips and that `invoke_raw_json` returns JSON
