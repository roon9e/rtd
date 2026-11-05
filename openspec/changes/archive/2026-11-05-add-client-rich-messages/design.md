# Design

## Context

Layer 232 already contains the `RichText`/`PageBlock` constructors in `rtd-tg`; only builders and
the request plumbing are missing.

## Goals / Non-Goals

**Goals**

- A tree builder for rich text and page blocks.
- An HTML/Markdown parser producing that tree.

**Non-Goals**

- Tables, math and footnote anchors in the first cut.

## Decisions

### D1. Builder mirrors the constructor tree

A `RichText` enum and a `PageBlock` enum wrap the generated constructors; `RichMessage` holds the
blocks, or the raw HTML/Markdown.

### D2. Raw HTML/Markdown is passed to the server

`InputRichMessage` has `inputRichMessageHTML`/`inputRichMessageMarkdown` variants, so the client
does not parse HTML/Markdown itself; `RichMessage::html`/`markdown` wrap the raw source and let
the server parse it. Structured content is built from `RichText`/`PageBlock` nodes.

## Risks / Trade-offs

- [Schema drift] Rich types are newer and may change; keep the builder thin over the generated
  constructors.
