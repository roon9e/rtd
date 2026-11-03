# Design

## Context

`Updates`/`Update` are already decoded and dispatched. `td` maintains independent pts/qts/seq
engines: each expects `current + 1`, buffers ahead-of-sequence updates in a gap buffer, and on a
timeout requests a difference. Channel updates carry their own `pts` per channel.

## Goals / Non-Goals

**Goals**

- No lost or reordered updates across a reconnect.
- Deterministic tests with a mock server producing gapped sequences.

**Non-Goals**

- The server side of difference generation.

## Decisions

### D1. Three independent sequence boxes

`sequenceBox` per stream (pts/qts/seq) with `state`, `gapBuffer`, and `gapTimeout`. `Handle`
either applies (`current + 1`), buffers (ahead), or triggers a difference (behind/duplicate).

### D2. Recovery is a request, not a guess

On a gap timeout, call `updates.getDifference { pts, qts, date, pts_total_limit }`. Match
`DifferenceSlice` (apply and repeat), `DifferenceEmpty` (advance), or `DifferenceTooLong`
(reset to state and warn) — mirroring `td`.

### D3. Channel state is per-channel and capped

A worker per channel calls `updates.getChannelDifference`; a semaphore caps concurrency
(`MaxChannelDifferenceConcurrency`). Access hashes are cached with a feeder.

## Risks / Trade-offs

- [Unbounded channel workers] → cap concurrency and evict idle channels.
- [State growth] → `StateStorage` trait with an in-memory default; persistence is separate.
