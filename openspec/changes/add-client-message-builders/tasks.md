# Tasks

## 1. Core

- [ ] 1.1 Add `Sender`/`RequestBuilder` with the standard options (`message/mod.rs`)
- [ ] 1.2 Add entity/text builders and HTML/Markdown parsing

## 2. Content

- [ ] 2.1 Add media senders (photo, document, video, audio, gif, sticker, poll, geo, contact) and media group
- [ ] 2.2 Add inline keyboard and inline-result builders

## 3. Verification

- [ ] 3.1 Verify a formatted message produces the expected entities (`message` tests)
- [ ] 3.2 Verify a send through a mock server returns the sent message
- [ ] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
