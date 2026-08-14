# Build and run Nyanbin from source

Install Docker with the Compose plugin, Node.js and pnpm from the repository pins, and Rust from `mise.toml`.

```sh
mise install
pnpm install --frozen-lockfile
pnpm run build
docker build --tag nyanbin:local .
NYANBIN_IMAGE=nyanbin:local docker compose up -d --wait
```

Nyanbin is then available on `http://127.0.0.1:8000`. Put a TLS reverse proxy in front before exposing it publicly; the browser fragment contains the decryption secret and must never traverse plaintext HTTP.

The default Compose deployment keeps Valkey in a tmpfs with persistence disabled. This protects bounded retention and avoids secrets on disk, but every Valkey restart removes all notes. `noeviction` is intentional: under memory pressure creates fail rather than silently deleting unrelated notes. Adjust `VALKEY_MAXMEMORY` and the `NYANBIN_*` lifecycle and rate limits for the host.

The application container runs as UID/GID 10001, drops all capabilities, uses a read-only root filesystem, and has only a small `/tmp` tmpfs. Check liveness at `/api/live` and dependency readiness at `/api/ready`.

Stop and remove the stack with:

```sh
docker compose down --remove-orphans
```
