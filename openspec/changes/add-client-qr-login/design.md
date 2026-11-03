# Design

## Context

`auth.exportLoginToken` returns `auth.LoginToken` (or `LoginTokenMigrateTo`), whose `token` is
`tg://login?token=...`. The scanning client calls `auth.acceptLoginToken`, and the exporting
client then calls `auth.importLoginToken` to obtain `auth.Authorization`. `td` wraps this in a
`QR` type with a token callback.

## Goals / Non-Goals

**Goals**

- Typed export/accept/import with `MigrateTo` handling.
- A polling loop with a token callback, deterministic under `ManualClock`.

**Non-Goals**

- QR image rendering.

## Decisions

### D1. Token represented as both bytes and URL

`LoginToken { token: Vec<u8>, url: String, expires: i32 }`; the URL is
`tg://login?token=<base64url(token)>`, matching `td`.

### D2. Migration on export

`auth.LoginTokenMigrateTo` triggers `Client::migrate` (or a DC switch once `add-client-dcs`
lands); until then, export surfaces the migration as a typed error.

## Risks / Trade-offs

- [Polling loop] → bounded by token expiry from the clock; no unbounded retry.
