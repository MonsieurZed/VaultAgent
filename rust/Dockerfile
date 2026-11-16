FROM rust:1.100.0-bookworm@sha256:ade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea AS base
WORKDIR /build
COPY rust ./

FROM base AS test
RUN cargo test --locked -j 2 && rm -rf target

FROM base AS builder
RUN cargo build --release --locked -j 2 -p vaultagent-server \
    && install -D target/release/vaultagent-server /out/vaultagent-server

FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /data /downloads /app && chown 1000:1000 /data
WORKDIR /app
COPY --from=builder /out/vaultagent-server /usr/local/bin/vaultagent-server
COPY LICENSE LICENSE-GPL.txt THIRD-PARTY.md ./
USER 1000:1000
EXPOSE 8080
HEALTHCHECK --interval=20s --timeout=4s CMD ["/usr/local/bin/vaultagent-server", "healthcheck"]
CMD ["/usr/local/bin/vaultagent-server"]
