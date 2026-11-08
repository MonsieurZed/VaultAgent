FROM node:22.23.4-bookworm-slim@sha256:5ec23b7c622b00547753bc6ffd78085dd868a68aeb41c8e0a1418f033e13d96d AS builder
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY fork ./fork
COPY linux/build-engine.mjs ./linux/build-engine.mjs
RUN node linux/build-engine.mjs

FROM node:22.23.4-bookworm-slim@sha256:5ec23b7c622b00547753bc6ffd78085dd868a68aeb41c8e0a1418f033e13d96d
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /build/runtime ./runtime
COPY linux ./linux
COPY remote-client ./remote-client
COPY LICENSE LICENSE-GPL.txt THIRD-PARTY.md ./
COPY fork ./sources/fork
USER 1000:1000
ENV NODE_ENV=production
EXPOSE 8080
HEALTHCHECK --interval=20s --timeout=4s CMD node -e "fetch('http://127.0.0.1:8080/healthz').then(r=>{if(!r.ok)process.exit(1)}).catch(()=>process.exit(1))"
CMD ["node", "linux/server.mjs"]
