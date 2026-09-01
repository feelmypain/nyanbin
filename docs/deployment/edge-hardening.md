# Edge and origin hardening

Copy-paste reference configuration for running a public Nyanbin instance behind Cloudflare (or a comparable CDN/WAF) with an origin nginx. Everything here is placeholder-based: replace `example.com`, `203.0.113.10`, and certificate paths with your own values. The rationale for each control is in the README's *Production hardening* section.

## Origin nginx

### Rate-limit zones (`http` context)

```nginx
# General per-IP API budget.
limit_req_zone $binary_remote_addr zone=nyanbin_api:10m rate=30r/s;

# Stricter budget for short-code resolution — the only guessable surface.
limit_req_zone $binary_remote_addr zone=nyanbin_short:10m rate=5r/s;
```

### Real client IP restoration (`http` context)

Declare **every** published Cloudflare CIDR (their list changes occasionally — re-sync it when you update nginx), then trust the `CF-Connecting-IP` header:

```nginx
# Repeat for each published Cloudflare IPv4 and IPv6 range:
set_real_ip_from 198.51.100.0/24;   # placeholder — use the real published ranges
set_real_ip_from 2001:db8::/32;     # placeholder — use the real published ranges

real_ip_header CF-Connecting-IP;
```

### Server block

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name example.com;

    # --- TLS ---
    ssl_certificate     /etc/nginx/tls/example.com.fullchain.pem;
    ssl_certificate_key /etc/nginx/tls/example.com.key.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers off;

    # --- Authenticated Origin Pulls ---
    # Only TLS clients presenting the Cloudflare origin-pull client
    # certificate may connect; direct-to-origin requests fail the handshake.
    ssl_client_certificate /etc/nginx/cloudflare-origin-pull-ca.pem;
    ssl_verify_client on;

    # --- Body size ---
    # NYANBIN_MAX_ENVELOPE_BYTES limits the *decoded* envelope; the JSON body
    # carries base64url (×4/3) plus field overhead. For the 1 MiB default,
    # 2m leaves comfortable slack. Scale this with your envelope limit.
    client_max_body_size 2m;

    # --- Stricter zone for short-code resolution ---
    location /api/short/ {
        limit_req zone=nyanbin_short burst=10 nodelay;
        limit_req_status 429;

        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Forwarded-Proto https;
    }

    # --- General API ---
    location /api/ {
        limit_req zone=nyanbin_api burst=60 nodelay;
        limit_req_status 429;

        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Forwarded-Proto https;
    }

    # --- Static frontend and note pages ---
    location / {
        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Forwarded-Proto https;
    }
}
```

Notes:

- `$remote_addr` is already the restored client address thanks to `real_ip_header CF-Connecting-IP`, so forwarding it as `X-Real-IP`/`X-Forwarded-For` gives the app the true client. Set `NYANBIN_TRUSTED_PROXY_CIDRS` to **only** this nginx (its loopback or container-network address) so the app accepts those headers from nowhere else.
- Do not add response caching, request coalescing, or retry-on-error for `/api/` — reveal consumes a read and must reach the app exactly as sent.
- Truncate note paths in access logs so a log leak cannot enumerate note IDs, e.g. with a mapped log variable that cuts `/api/notes/<id>…` down to `/api/notes/…`.

## Firewall

Accept 443 only from the published Cloudflare IP ranges and drop everything else, so the origin cannot even be handshaken by scanners:

```sh
# nftables sketch — populate the set from Cloudflare's published ranges.
nft add set inet filter cf_ranges '{ type ipv4_addr; flags interval; }'
nft add rule inet filter input tcp dport 443 ip saddr @cf_ranges accept
nft add rule inet filter input tcp dport 443 drop
```

Keep the origin IP (e.g. `203.0.113.10`) out of DNS, certificates with revealing SANs, and anything else public. If it leaks, rotate it.

## Cloudflare dashboard checklist

| Setting | Value | Why |
| --- | --- | --- |
| SSL/TLS mode | **Full (strict)** | The edge validates the origin certificate; anything weaker allows on-path substitution of the origin |
| Authenticated Origin Pulls | **Enabled zone-wide** | Pairs with `ssl_verify_client on` at the origin; both halves are required |
| Rate limiting rule | Per-IP rule on `/api/*`; start near 300 requests/minute per IP and tune from the app's own 429 rates | Absorb volumetric floods at the edge before they consume origin capacity |
| Bot Fight Mode / challenges | Scope challenges to human-facing short-link pages (`/s/*`) **only**; never challenge `/api/notes/*` or other `/api/*` paths | The `nyanbin` CLI and non-browser clients cannot solve a browser challenge; a zone-wide challenge silently breaks them |
| Cache rules | **Bypass cache for `/api/*`** | Reveal is consuming and commit is stateful; a cached API response is a correctness and privacy failure |
| Always Use HTTPS | Enabled | No plaintext window for the share URL path portion |

## Verification

After any edge or origin change, run the operator verification checklist in the README's *Production hardening* section — in particular: direct-to-origin requests must fail the TLS handshake, edge-routed requests must succeed, and a burn-after-reading note must not survive a second reveal through the edge.
