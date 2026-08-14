# Nyanbin behind Traefik

This example assumes an existing external Docker network named `proxy`, a TLS entrypoint named `secure`, and a certificate resolver named `le`. Replace `nyanbin.example` before deployment.

```yaml
networks:
  proxy:
    external: true

services:
  valkey:
    image: valkey/valkey:8.1-alpine
    command: ["valkey-server", "--save", "", "--appendonly", "no", "--maxmemory", "256mb", "--maxmemory-policy", "noeviction"]
    tmpfs:
      - /data:size=256m,mode=0700
    healthcheck:
      test: ["CMD", "valkey-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 10

  app:
    image: ${NYANBIN_IMAGE:-nyanbin:latest}
    environment:
      NYANBIN_REDIS_URL: redis://valkey:6379/
      # Set only when the proxy address is stable and trusted.
      NYANBIN_TRUSTED_PROXY_CIDRS: ""
      # Per-client writes share a bucket across this many IPv6 prefix bits (/64 by default).
      NYANBIN_RATE_LIMIT_IPV6_PREFIX_BITS: "64"
      # Global writes per window cap abuse even when clients rotate addresses.
      NYANBIN_RATE_LIMIT_GLOBAL_REQUESTS: "300"
    depends_on:
      valkey:
        condition: service_healthy
    networks: [default, proxy]
    read_only: true
    tmpfs: [/tmp]
    cap_drop: [ALL]
    security_opt: [no-new-privileges:true]
    labels:
      - traefik.enable=true
      - traefik.docker.network=proxy
      - traefik.http.routers.nyanbin.rule=Host(`nyanbin.example`)
      - traefik.http.routers.nyanbin.entrypoints=secure
      - traefik.http.routers.nyanbin.tls=true
      - traefik.http.routers.nyanbin.tls.certresolver=le
      - traefik.http.services.nyanbin.loadbalancer.server.port=8000
```

Start it with `docker compose up -d` and confirm that `/api/ready` returns HTTP 200. Valkey is deliberately ephemeral: restarts discard notes. Keep the service off public networks, retain `noeviction`, and size `--maxmemory` for the configured envelope and traffic limits.

`NYANBIN_RATE_LIMIT_REQUESTS` limits each normalized client bucket, while `NYANBIN_RATE_LIMIT_GLOBAL_REQUESTS` limits address rotation across the instance; reservation and commit each have their own counters under both ceilings. IPv4 clients are bucketed by address; IPv6 clients are bucketed by `NYANBIN_RATE_LIMIT_IPV6_PREFIX_BITS` (0–128, default 64) so privacy-address rotation within a subnet does not multiply capacity. Choose the IPv6 prefix for the networks serving your users, and size the global ceiling for the instance rather than per replica because counters live in shared Valkey.
