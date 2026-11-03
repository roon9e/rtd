# Proposal

## Why

Bots sign in with a token, not a phone code: `auth.importBotAuthorization`. `rtd-client` exposes
only the user code/sign-in path, so a bot cannot authenticate. `td` provides `Client.Auth().Bot`
(`telegram/auth/bot.go`).

## What Changes

- Add `Client::bot_sign_in(token)`: parse `id:secret`, call
  `auth.importBotAuthorization { flags: 0, api_id, api_hash, bot_auth_token }`, return
  `auth.Authorization`.
- Accept the token from an option or environment for CLI use.

## Capabilities

### New Capabilities

- `client-bot-auth`: bot-token authorization.

## Impact

- **Crates**: `rtd-client` (`client.rs`).
- **Non-goals**: bot API helpers (sending messages, keyboards) — those are message builders.
