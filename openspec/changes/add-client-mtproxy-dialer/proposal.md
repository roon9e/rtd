# Proposal

## Why

`rtd-transport` implements all MTProxy primitives (`obfuscated2` client+server, `faketls`
client+server, `secret` parsing plain/`dd`/`ee`), but `rtd-client::connect_tcp` only ever
applies `Obfuscated2::handshake`; it never selects FakeTLS and exposes no dialer abstraction. A
client therefore cannot connect through an MTProxy with an `ee` (TLS) secret. `td` has a
dedicated `mtproxy/obfuscator` package with `Obfuscated2()`/`FakeTLS()` dialers.

## What Changes

- Add a `Dialer`/`Obfuscator` abstraction that composes the obfuscation layers:
  `obfuscated2`, or `faketls` over `obfuscated2` (matching `td`'s `tls` composition).
- Select the layer from a parsed `Secret` (plain, `dd` secured, `ee` FakeTLS + cloak host).
- Support multiple endpoints with fallback, and carry the DC salt into the handshake.
- Use it from `rtd-client` so `connect_tcp` (or a new `dial`) honours `ee` secrets.

## Capabilities

### New Capabilities

- `client-mtproxy-dialer`: MTProxy-aware dialing that selects the obfuscation layer from a
  secret.

## Impact

- **Crates**: `rtd-transport` (a dialer/obfuscator module) and `rtd-client` (`transport.rs`).
- **Non-goals**: an MTProxy *server* (proxy that forwards clients to a DC with proxy tags).
