# Proposal

## Why

A real client could not complete the handshake with `rtd-edge` even though the exchange is
otherwise correct. Decoding the client's (deobfuscated) frames showed it selects **DC labels 3 and
4** (not 2): `req_DH_params` carried `p_q_inner_data_dc { dc = 3|4 }`, and `rtd-edge` rejected it
with `WrongDc`, so the server never replied `server_DH_params_ok` and the client gave up. The
reference edge runs with `strict_dc_check: false` — a single physical backend answers for **every**
wire DC label, and the label never changes key persistence or session identity.

## What Changes

- `rtd-exchange`'s `ServerConfig` gains `strict_dc: bool`; when false the exchange accepts a
  client-selected DC label instead of comparing it to the served DC.
- `rtd-edge` sets `strict_dc: false`, matching the reference edge's default.

## Capabilities

### Modified Capabilities

- `edge-service`: the edge accepts the client's datacenter label like the reference edge.

## Impact

- **Crates**: `rtd-exchange` (config + exchange), `rtd-edge` (the exchange strategy), and the
  exchange tests.
- **Compatibility**: matches gramsrv v2's `public.strict_dc_check: false`.
- **Multi-DC caveat (do not forget)**: accepting every label is a **single-backend workaround**.
  In a real multi-DC runtime the DC label is the *routing identity* — it selects the backend and
  is part of the key/session identity — so it must be exact; an unknown label must be routed or
  refused, never silently served by the local DC. Multi-server clients multiplex many accounts in
  one app and may use labels well above 5 (even tens), so the multi-DC runtime must hold a real
  `label -> backend` map rather than treating any label as local.
