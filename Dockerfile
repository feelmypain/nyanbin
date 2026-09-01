# syntax=docker/dockerfile:1

FROM node:24-alpine AS web
ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH
RUN corepack enable
WORKDIR /build
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY packages/cli/package.json packages/cli/package.json
COPY packages/frontend/package.json packages/frontend/package.json
COPY packages/backend/package.json packages/backend/package.json
RUN pnpm install --frozen-lockfile --ignore-scripts
COPY packages/cli packages/cli
COPY packages/frontend packages/frontend
COPY packages/backend/openapi.json packages/backend/openapi.json
RUN pnpm --filter ./packages/frontend exec svelte-kit sync \
    && pnpm --filter ./packages/cli build \
    && pnpm --filter ./packages/frontend build

FROM rust:1.95-alpine AS backend
RUN apk add --no-cache alpine-sdk libc-dev openssl-dev
WORKDIR /build
COPY packages/backend ./
RUN RUSTFLAGS="-Ctarget-feature=-crt-static" cargo build --locked --release

FROM alpine:3.22 AS runtime
RUN apk add --no-cache ca-certificates curl libgcc \
    && addgroup -S -g 10001 nyanbin \
    && adduser -S -D -H -u 10001 -G nyanbin nyanbin
WORKDIR /app
COPY --from=backend --chown=nyanbin:nyanbin /build/target/release/nyanbin /app/nyanbin
COPY --from=backend --chown=nyanbin:nyanbin /build/target/release/nyanbin-admin /usr/local/bin/nyanbin-admin
COPY --from=web --chown=nyanbin:nyanbin /build/packages/frontend/build /app/frontend
COPY --chown=nyanbin:nyanbin LICENSE THIRD_PARTY_NOTICES DEPENDENCY_LICENSES.csv /app/
ENV NYANBIN_FRONTEND_PATH=/app/frontend \
    NYANBIN_LISTEN_ADDR=0.0.0.0:8000 \
    NYANBIN_REDIS_URL=redis://valkey:6379/
USER 10001:10001
EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl --fail --silent --show-error http://127.0.0.1:8000/api/live >/dev/null || exit 1
ENTRYPOINT ["/app/nyanbin"]
