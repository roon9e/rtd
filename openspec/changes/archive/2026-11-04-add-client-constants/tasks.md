# Tasks

## 1. Constants

- [x] 1.1 Add well-known user IDs, upload limits and test credentials to `constant.rs`
- [x] 1.2 Add `TdlibPeerId` with user/chat/channel constructors and `to_plain`
- [x] 1.3 Add `is_user`/`is_chat`/`is_channel`/`is_monoforum` predicates

## 2. Verification

- [x] 2.1 Verify classification matches `td`'s test vectors
- [x] 2.2 Verify every classified id round-trips through `to_plain` and the matching constructor
