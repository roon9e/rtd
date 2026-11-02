# Design

## Context

`rtd_transport::secret::Secret::parse` yields plain/`dd`/`ee` modes and derives the obfuscated2
and faketls keys. `faketls::client_handshake` and `Obfuscated2::handshake` are both implemented
over `Read + Write`. `td` composes them as `tls{ftls, obfs2}` where writes go through the
obfuscated2 layer wrapped around FakeTLS.

## Goals / Non-Goals

**Goals**

- One abstraction that dials a plain, `dd`, or `ee` MTProxy and returns a framed stream.
- Layer composition identical to `td` (`faketls` inner, `obfuscated2` outer).

**Non-Goals**

- Proxy-server semantics, promo tags, or `dd` proxy-secret mediation.

## Decisions

### D1. `Dialer` trait with a `Secret`-aware default

```
pub trait Dialer { fn dial(&self, addr: &str, dc: u16, secret: &Secret) -> Result<Box<dyn ReadWrite>>; }
```

`DefaultDialer` maps `SecretMode::Plain` → obfuscated2 only, `SecretMode::Secured` (`dd`) →
obfuscated2 with the secured key, `SecretMode::FakeTls` (`ee`) → faketls handshake then
obfuscated2, matching `td`'s `obfuscator` package.

### D2. `encode` order mirrors `td`

Writes flow `obfuscated2 → faketls → socket`; reads reverse. The framing codec is applied above
the whole stack, and the layer carries the obfuscated transport tag.

### D3. Endpoints, not a single address

`Dialer::dial_all(endpoints, dc, secret)` races/falls back across endpoints; `rtd-client`
passes the resolved endpoint list.

## Risks / Trade-offs

- [FakeTLS handshake mismatch with modern clients] → pin the ClientHello to the `faketls`
  golden vectors already in `rtd-transport`; add a `td`-compat test.
