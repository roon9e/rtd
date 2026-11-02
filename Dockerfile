# syntax=docker/dockerfile:1
#
# Container image for the rtd MTProto edge. Build from the repository root:
#
#   docker build -f Dockerfile -t rtd-edge:local .
#
# The edge is a client-facing MTProto sidecar that fronts a core over the
# `telesrv.coreexec.v1.CoreExecService` gRPC contract (see crates/rtd-edge-proto).
# Supply the RSA key, bind address, core endpoint and core token at run time.

FROM rust:1.99-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p rtd-edge

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home rtd
COPY --from=build /src/target/release/rtd-edge /usr/local/bin/rtd-edge
USER 10001
ENTRYPOINT ["/usr/local/bin/rtd-edge"]
