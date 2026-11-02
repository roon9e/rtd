# Proposal

## Why

OwpenGram-family clients (the ones a gramsrv deployment serves) fetch the server descriptor from
the MTProto port before their first handshake:

```
GET http://<host>:<port>/owpengram/server-info    -> JSON { dc_id, rsa_public_key_pem, name, description, has_icon }
```

The descriptor carries the server's RSA public key. Without it a client has no key to match the
`server_public_key_fingerprints` in the exchange `resPQ`, sends `req_pq`/`req_pq_multi`, receives
`resPQ`, and aborts before `req_DH_params` — it cannot complete the handshake. `rtd-edge`'s
same-port HTTP currently only upgrades WebSocket, so any `GET` is dropped.

## What Changes

- `rtd-server`'s ingress gains an optional same-port HTTP [`ServerInfo`] descriptor and serves
  `GET`/`HEAD /owpengram/server-info` (JSON with `dc_id` + PKCS#1 `rsa_public_key_pem`) and
  `/owpengram/server-icon` (icon bytes, or 404 when none), matching gramsrv v2's
  `internal/serverinfo`; other paths 404 (WebSocket `/apiws*` still upgrades).
- `rtd-edge` builds the descriptor from its configured datacenter and RSA key.

## Capabilities

### Modified Capabilities

- `edge-service`: the edge serves the public server descriptor on the MTProto port.

## Impact

- **Crates**: `rtd-server` (ingress), `rtd-edge` (service/config).
- **Non-goals**: operator name/description/icon overrides (served empty / 404 for now).
- **Compatibility**: the response is plain HTTP/1.1 with an explicit `Content-Length` (the
  desktop client's minimal parser cannot decode chunked bodies).
