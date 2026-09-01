# Nyanbin operations runbook

A generic incident playbook for operators of a public Nyanbin instance. It assumes the reference deployment from the README: an edge CDN/WAF, an origin nginx with Authenticated Origin Pulls, the app container, and Valkey. All hostnames and addresses below are placeholders.

## Observation surface

Nyanbin intentionally exposes only privacy-safe signals:

- **`ctr:*` minute counters** in Valkey — per-operation aggregate counts of requests, rejections (rate-limited, storage-pressure, disabled), and short-resolve hits/misses, bucketed per minute. No client identity is attached.
- **`nyanbin-admin stats`** — run inside the app container (`docker exec <app-container> nyanbin-admin stats`); prints the current counters, storage occupancy, active switches, and blocked-bucket count.
- **`/api/ready`** — `200` only when Valkey is usable; the canonical dependency probe.
- **`/api/live`** — process liveness, independent of Valkey.
- Reverse-proxy status-code and latency metrics (aggregate only).

### What is never logged

By design, and it must stay this way through any incident:

- request or response **bodies** and **envelopes** (ciphertext included);
- **raw client IPs** in application state — rate-limit and quota buckets are pseudonymous;
- **note-ID ↔ client joins** — nothing correlates who created or read which note;
- **tokens** of any kind (delete tokens, their verifiers appear only in storage);
- **URLs with fragments** — the fragment is the key for secret-keyed notes.

If a diagnostic step would require adding any of the above, the answer is no. Escalate with aggregates.

## Symptom → diagnosis → action

| Symptom | Diagnosis | Action |
| --- | --- | --- |
| Commit volume spikes; `ctr:*` commit counters and stored bytes climbing fast | Write flood (spam or storage-fill attempt) | `nyanbin-admin switch writes_small_only on` to brownout large commits while small legitimate notes keep working; identify the offending pseudonymous bucket(s) from rejection counters and `nyanbin-admin block <bucket>`; revert the switch once counters normalize |
| Storage occupancy ≥ 90% | Occupancy guard has auto-browned-out large commits (`507 storage_pressure` for envelopes over 64 KiB) | Expected automatic behavior — verify with `nyanbin-admin stats`; watch the trend |
| Storage occupancy approaching 98% | Full commit brownout imminent (all commits → `507`) | `nyanbin-admin switch writes_off on` to fail cleanly instead of at the memory ceiling; then either raise Valkey memory (`VALKEY_MAXMEMORY`) and `NYANBIN_STORAGE_BUDGET_BYTES`, or lower `NYANBIN_MAX_EXPIRES_IN` so existing notes drain faster; re-enable writes once occupancy recedes |
| Short-resolve miss ratio spikes (`ctr:*` short-resolve misses ≫ hits) | Short-code enumeration attempt (codes are 6 digits) | The `resolve_hardened` switch auto-arms and tightens the global resolve ceiling; if the pattern persists, `nyanbin-admin switch short_off on` — bare password links (`/note/{id}`) are unaffected, only `/s/{code}` resolution stops |
| Operator takedown decision for a specific note | Note ID needs removal | `nyanbin-admin revoke <id> --yes`; optionally `nyanbin-admin block <bucket>` if a rejection pattern identifies the creating bucket. Never attempt to decrypt or inspect content — the operator cannot, and that is the point |
| `/api/ready` returns `503`; all writes and reveals fail | Valkey down or unreachable | Everything fails closed by design — no data-integrity risk. Restore Valkey (container restart, memory, network); the app self-recovers without restart once Valkey returns. Remember the default deployment is ephemeral: notes stored before a Valkey restart are gone |
| Requests appear at the origin without edge headers, or origin IP shows up in scans | Suspected origin bypass | Verify `ssl_verify_client on` and the origin-pull CA path in nginx, verify the firewall still allowlists only edge IP ranges, then rotate the origin IP and update edge configuration |
| Sustained `429` on one operation for all clients | Per-operation global cap tripping (attack or organic growth) | Check `ctr:*` for which operation; if organic, raise the matching `NYANBIN_RATE_LIMIT_GLOBAL_<OP>_REQUESTS`; if hostile, block the dominant buckets and leave the cap alone |

## Alert-threshold suggestions

Tune to instance size; these are sane starting points:

| Signal | Suggested trigger |
| --- | --- |
| Global per-operation cap trips | Sustained for > 3 minutes |
| Storage occupancy | Warn at 80%, page at 90%, critical at 98% |
| Short-resolve miss ratio | Trip when misses dominate hits over a 5-minute window |
| Valkey operation latency | Warn approaching `NYANBIN_REDIS_TIMEOUT_MS` |
| 5xx ratio at the edge or origin | Warn > 1% of requests over 5 minutes |
| `/api/ready` | Page on any failing probe |

## Kill-switch reference

All switches are Valkey-backed and take effect immediately without redeploy or restart:

| Switch | Effect | Typical trigger |
| --- | --- | --- |
| `writes_off` | Reserve and commit → `503 writes_disabled` | Storage exhaustion, active incident |
| `writes_small_only` | Large commits → `507 storage_pressure` | Write flood, ≥ 90% occupancy pre-emption |
| `short_off` | Short create and resolve → `503 short_disabled` | Persistent short-code enumeration |
| `resolve_hardened` | Tighter global short-resolve ceiling | Auto-arms on miss-ratio spikes; may be held on manually |

Toggle with `nyanbin-admin switch <name> on|off` inside the app container. Reads, reveals, deletes, and existing password links keep working under every switch except a full Valkey outage.
