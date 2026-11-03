# Frozen changes (server / edge)

`rtd` is now a **library-only** project: a Rust implementation targeting full
`td` (gotd) parity. It was previously exercised as an MTProto edge inside a
gramsrv stack, and that edge/server work is out of scope.

The change proposals in this directory are **frozen**, not deleted: they
describe server and edge capabilities that are deferred until a future,
separate full Rust server replaces the compatibility `rtd-edge` image. Their
deployed capability specs remain in `openspec/specs/` (`server-rpc`,
`server-edge`, `edge-service`, `multi-dc`).

Frozen changes:

- `add-server-storage`, `add-server-observability`, `add-server-rate-limit`,
  `add-server-exchange-params`
- `add-edge-caches`, `add-edge-config-parity`, `add-edge-delivery-fabric`,
  `add-edge-file-fastpath`, `add-edge-identity-metrics`,
  `add-edge-location-registry`, `add-edge-public-advert`,
  `add-edge-same-port-mux`, `add-edge-session-lifecycle`,
  `add-edge-updates-projection`, `add-edge-websocket-origins`

The `rtd-server`, `rtd-edge` and `rtd-edge-proto` crates remain in the tree,
frozen at their current state, so the compatibility image stays buildable. No
new server/edge work is planned here.
