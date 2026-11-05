# Tasks

## 1. Builders

- [x] 1.1 Add `RichText` builders (plain, bold, italic, fixed, link, concat)
- [x] 1.2 Add paragraph and heading page-block builders
- [x] 1.3 Add `RichMessage` and plumb `rich_message` through `RequestBuilder`/`edit_rich_message`

## 2. Parsing

- [x] 2.1 Raw rich HTML/Markdown are encoded as `inputRichMessageHTML`/`inputRichMessageMarkdown` (parsing is server-side)

## 3. Verification

- [x] 3.1 Verify a built rich message maps to its input constructor
- [x] 3.2 Verify paragraph/heading build the expected blocks and attach to a send
