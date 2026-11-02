# Design

## Context

`Client::invoke` already sends any `rtd_tg::Method` wrapped in `invokeWithLayer` and returns
the decoded `rtd_tg::Object`. `auth.sendCode#a677244f` returns `auth.SentCode` and
`auth.signIn#8d52a951` returns `auth.Authorization`; both live in the canonical model, so the
flow is a thin typed layer over the existing machinery.

## Goals / Non-Goals

**Goals**

- Typed `send_code`/`sign_in` over the standard two-call login flow.
- A deterministic test proving the flow end to end.

**Non-Goals**

- 2FA (`checkPassword`), sign-up, QR login, and persistent authorization state.

## Decisions

### D1. Thin typed wrappers, not a state machine

`send_code`/`sign_in` build the request, call `invoke`, and match the expected `Object`
variant, returning the typed value or [`crate::Error::Unexpected`] when the server answers
with a different constructor. No authorization state is retained; the caller drives the flow
(including 2FA), and the async façade reaches any method through its generic `invoke`.

### D2. A mock server dispatches on the unwrapped method id

Every client call is wrapped in `invokeWithLayer` (and the first in `initConnection`), so the
test server unwraps those wrappers and matches the inner method's wire id: `help.getConfig`
answers `Bool`, `auth.sendCode` answers `auth.SentCode` (`sentCodeTypeApp`), and `auth.signIn`
answers `auth.Authorization` (`authorizationSignUpRequired`, the smallest variant), avoiding
fabricating a `User`.

## Risks / Trade-offs

- [A thin wrapper is easy to outgrow] → the flow (2FA, sign-up) stays in the caller; the
  wrapper only types the two calls everyone makes.
