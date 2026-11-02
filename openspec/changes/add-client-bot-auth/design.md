# Design

## Context

`auth.importBotAuthorization` takes `api_id`, `api_hash` and `bot_auth_token` and returns
`auth.Authorization`. The client already knows `api_id`/`api_hash` from `initialize`; only the
token is new. `td` splits `token` on `:` for display but sends the whole token.

## Goals / Non-Goals

**Goals**

- One call that authorizes a bot and returns the typed authorization.
- Deterministic test over a mock server.

**Non-Goals**

- Userbot login, session strings.

## Decisions

### D1. Reuse `invoke` with an explicit `invokeWithLayer` wrapper

`bot_sign_in` builds `auth.import_bot_authorization` and invokes it through the existing
wrapper path, matching the layer of the other calls.

### D2. Token source is an option

`ClientOptions.bot_token` (and `RTD_BOT_TOKEN` env) so the CLI can run token-only.

## Risks / Trade-offs

- [Leaking the token in logs] → never log the token; the error path reports only
  `AUTH_TOKEN_INVALID` style types.
