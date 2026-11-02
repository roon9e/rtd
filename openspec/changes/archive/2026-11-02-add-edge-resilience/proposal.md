# Proposal

## Why

Running `rtd-edge` as a container in a gramsrv v2 stack, clients cannot connect: the edge
process exits and Docker restarts it, so the MTProto port is intermittently not listening. Two
startup/runtime failures cause it:

1. **A single bad connection kills the whole edge.** A client or probe that connects and closes
   before its transport header arrives makes `Ingress::accept` return an error
   (`listener failed: io error: early eof`); [`Server::serve`] propagates it and the process
   exits.
2. **Startup depends on core reachability.** `build_core` dials the core eagerly
   (`GrpcCore::connect`); if the core is not yet up (it takes ~30 s after a stack restart) the
   edge exits with `core endpoint failed: transport error` and restart-loops until the core
   happens to be ready during a restart window.

## What Changes

- `Server::serve` treats an accept error as a per-connection failure and keeps accepting.
- `rtd-edge`'s `build_core` uses a lazy core channel, so the edge starts regardless of core
  readiness and serves once the core is reachable.

## Capabilities

### Modified Capabilities

- `edge-service`: the edge stays listening across a bad connection and a temporarily unavailable core.

## Impact

- **Crates**: `rtd-server` (`actor::Server::serve`) and `rtd-edge` (`service::build_core`).
- **Non-goals**: retrying the authorization-key exchange or queuing; individual requests still
  surface a core-unavailable error until the core is up.
