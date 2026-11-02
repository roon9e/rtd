# Proposal

## Why

After `initialize`, a client must authorize before it can do anything useful: send a login
code (`auth.sendCode`) and sign in with it (`auth.signIn`). `rtd-client` can already invoke
any `rtd_tg::Method`, but the two-call authorization flow is not expressed, and its result
types are not surfaced.

## What Changes

- `Client::send_code` and `Client::sign_in`: typed helpers over `auth.sendCode`/`auth.signIn`
  that build the request, invoke it, and return the decoded `AuthSentCode`/
  `AuthAuthorization` (or a typed error when the server answers with something else).
- A mock-server test that drives `initialize` -> `sendCode` -> `signIn` and asserts the typed
  results, proving the flow over the encrypted session.

## Capabilities

### New Capabilities

- `client-authorization`: the client-side login-code and sign-in calls.

## Impact

- **Crates**: `rtd-client` only (a convenience API plus tests); no wire changes.
- **Non-goals**: 2FA/SRP (`auth.checkPassword`), sign-up, and QR login; the async façade
  already reaches these methods through its generic `invoke`.
