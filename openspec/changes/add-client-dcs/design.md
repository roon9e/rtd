# Design

## Context

`td` models DCs as `dcs.List` with options (`PreferIPv6`, `TCPObfuscatedOnly`, `NoObfuscated`)
and three resolver kinds, plus a DNS resolver that decodes an encrypted TXT record
(RSA + AES-CBC). Migration is driven by `_MIGRATE` errors in `invoke`.

## Goals / Non-Goals

**Goals**

- Resolve a DC to one or more dialable endpoints with a transport selected per options.
- Follow migrate errors and transfer authorization across DCs.

**Non-Goals**

- Multi-DC server routing.

## Decisions

### D1. `Resolver` returns `(Protocol, Endpoint)` pairs

`Resolver::resolve(dc, kind)` returns an ordered list of `Endpoint { ip, port, protocol }`;
`Protocol` ∈ {Obfuscated2, FakeTls, WebSocket, Plain}. The pool dials them in order via the
appropriate dialer.

### D2. Migrate errors are handled at the invoke boundary

When a result is an `rpc_error` whose type is `*_MIGRATE_*`, parse the DC, switch the pool's
target, and (for user accounts) export/import authorization (`auth.exportAuthorization`/
`auth.importAuthorization`), mirroring `td`'s `dc_transfer.go`.

### D3. DNS config is verified against vendored keys

Decode the encrypted TXT config with the vendored RSA keys (`rtd-crypto`), matching `td`'s DNS
resolver.

## Risks / Trade-offs

- [Racing dials can waste sockets] → bound concurrency and cancel losers.
