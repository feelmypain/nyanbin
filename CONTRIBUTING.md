# Contributing to Nyanbin

## Requirements

- [mise](https://mise.jdx.dev) for the pinned Node.js, pnpm, and Rust toolchains
- Docker with the Compose plugin for Valkey and end-to-end tests

## Setup

```sh
mise install
pnpm install --frozen-lockfile
```

## Development

```sh
pnpm run dev
```

This starts ephemeral Valkey plus the backend, web client, and CLI development processes. The web client is served at `http://127.0.0.1:3000`. Never use real secrets in development logs or fixtures.

## Checks

```sh
pnpm run check
cargo fmt --manifest-path packages/backend/Cargo.toml --check
cargo clippy --manifest-path packages/backend/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path packages/backend/Cargo.toml --locked
pnpm run test:local
```

Run the complete browser matrix with `pnpm test`. The Playwright harness builds and starts the read-only development Compose stack automatically.

Changes to the envelope or API contract must update shared browser/Node types, deterministic vectors, backend contract tests, and browser↔CLI interoperability coverage together. Tests must use synthetic secrets and must not contact third-party services.

## Releases

1. Update versions and `CHANGELOG.md`.
2. Commit the generated lockfiles.
3. Create and push a signed `v<semver>` tag.

The generic release workflow builds a CLI archive, attaches it to the repository release, and publishes the multi-architecture container under the repository's own GHCR namespace. It never targets an upstream image or credential.
