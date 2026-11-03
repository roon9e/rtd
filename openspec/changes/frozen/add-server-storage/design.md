# Design

## Context

`rtd-server::core::Core` already abstracts auth-key and layer-evidence access. `LocalCore`
implements it in memory; `GrpcCore` delegates to a remote core. The gap is a durable first-party
backend and a shared store for multi-DC.

## Goals / Non-Goals

**Goals**

- Persist auth keys, layer evidence and (later) delivery state.
- Keep the `Core`/storage traits object-safe and testable.

**Non-Goals**

- Query optimization; multi-region replication.

## Decisions

### D1. Traits over async, tokio-based

`AuthKeyStore`, `LayerEvidenceStore`, `DeliveryStore` as async traits; in-memory impls live in
`rtd-server` and are the default for tests.

### D2. PostgreSQL via `sqlx`

A `PostgresStore` using `sqlx` migrations (mirroring gramsrv's `deploy/migrations`), with Redis
for short-lived leases/coordination. No `unsafe`; queries are static.

### D3. Edge selects the backend

`rtd-edge` gains `--storage` / `--database-url` / `--redis-url`, defaulting to in-memory when
unset (so single-process tests keep working).

## Risks / Trade-offs

- [New heavy deps] → isolate behind a feature/optional crate so the sans-IO core stays clean.
- [Schema drift] → keep migrations in-repo and versioned.
