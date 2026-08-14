# Nyanbin implementation plan

Last updated: 2026-08-14

This file is the working checklist and decision log for the Nyanbin fork. Checked items are verified against the current tree or upstream source. Open items must be completed before the first release.

## Objective

Build **Nyanbin**, an original blue/kawaii, self-hostable, zero-knowledge paste and file sharing service. Use Cryptgeon's Rust/Axum + Svelte + CLI foundation and reimplement the strongest PrivateBin ideas without copying its PHP/JavaScript implementation or artwork.

## Upstream audit

- [x] Pulled Cryptgeon at `1f180a2e53b79dce4e201e4ec2fcf5201538c4b2` (2026-06-25).
- [x] Pulled PrivateBin at `88cf90534de33f950411c12cdee87d841d97947c` (2026-08-10).
- [x] Audited all Cryptgeon backend, frontend, CLI, tests, deployment, workflow, and license surfaces.
- [x] Audited all PrivateBin application code, client crypto, storage adapters, templates, tests, configuration, docs, security history, and licenses.
- [x] Studied WidgetStar's rendered desktop/mobile layout and public CSS as mood reference only.
- [x] Chose Cryptgeon as the technical foundation: modern typed clients, compact Rust server, Valkey lifecycle store, static deployment, and MIT license.
- [x] Chose conceptual reimplementation of PrivateBin features: strict envelopes, encrypted file metadata, authenticated metadata, delete capabilities, rich formats, safer previews, rate limiting, headers, explicit burn gate, and stronger threat-model copy.

## Security and protocol decisions

- [x] Keep a 256-bit random link secret in the URL fragment; fragments are not sent in normal HTTP requests.
- [x] Optional password is a second factor combined with the random link secret; it never replaces link entropy.
- [x] Define one Nyanbin v1 envelope. Do not claim Cryptgeon or PrivateBin wire compatibility.
- [x] AES-256-GCM, 12-byte random IV, 128-bit authentication tag, and authenticated public header/policy.
- [x] Encrypt content kind, formatter, text, filenames, MIME hints, sizes, and attachment bytes as one private payload.
- [x] PBKDF2-HMAC-SHA-256 at 600,000 iterations with a 16-byte salt for password mode. Parameters are fixed and bounded by v1.
- [x] Every note has an absolute expiry; an optional read cap is enforced atomically. `maxReads = 1` is burn-after-reading.
- [x] Reveal is an explicit POST action. Passive navigation, existence checks, previews, robots, and link scanners never consume a read.
- [x] Reveal consumes before local decryption. The UI must warn that a wrong password or broken link can spend a read.
- [x] Creation returns an independent random delete capability; only its SHA-256 verifier is stored.
- [x] No external URL shortener, analytics, third-party runtime asset, remote Markdown resource, or arbitrary operator HTML on crypto pages.
- [x] Do not ship discussions in v1. Encrypted multi-writer comments require a separate protocol and conflict with one-read semantics.
- [x] Do not ship never-expiring notes or multiple storage engines in v1. Bounded retention and one atomic Valkey implementation are deliberate.

## Backend

- [x] Replace the legacy note model with a closed, versioned public envelope and lifecycle policy.
- [x] Validate canonical base64url fields, ID/token lengths, body bounds, and unknown fields without panics.
- [x] Generate IDs and delete capabilities with checked CSPRNG calls.
- [x] Create with one atomic Valkey script: collision-safe insert plus TTL.
- [x] Reveal with one atomic Valkey script: expiry check, read decrement, terminal deletion, and one returned envelope across replicas.
- [x] Delete through a constant-time checked hashed capability.
- [x] Remove the process-local lock map and synchronous per-request Redis connections.
- [x] Add async Redis connection management and command timeouts.
- [x] Add separate `/api/live` and dependency-aware `/api/ready` endpoints.
- [x] Add configurable create rate limiting with bounded, pseudonymous client buckets and trusted-proxy controls.
- [x] Add strict CSP and security headers to every response.
- [x] Expose only non-secret capabilities and safe URL/text branding values.
- [x] Add backend contract tests for validation, atomicity, TTL, deletion, readiness, headers, rate limiting, and malformed storage.

## Shared crypto and CLI

- [x] Replace Occulto-dependent legacy adapters with a typed Nyanbin v1 protocol module.
- [x] Publish deterministic browser/Node vectors for text, Unicode, files, password/no-password, tampering, and wrong credentials.
- [x] Use direct typed-array base64url utilities; reject malformed encodings before expensive work.
- [x] Keep the private manifest wholly encrypted and authenticate note ID/version/lifecycle policy as AAD.
- [x] Rebrand the package, executable, environment variables, URL parsing, help, and errors to Nyanbin.
- [x] Support CLI create/open/delete for text, source, Markdown, multiple files, expiry, read cap, and password.
- [x] Preserve browser↔CLI byte-for-byte interoperability as a release gate.

## Frontend and product

- [x] Build an original blue-paper design system: ink navy, cornflower blue, tactile keylines, compact panels, and geometric cat motifs.
- [x] Use no copied WidgetStar logo, mascot, texture, sprites, icons, geometry bundle, or proprietary artwork.
- [x] Build a wide composer with text/source/Markdown modes, preview, mixed attachments, drag/drop/paste, exact limits, and per-file removal.
- [x] Build grouped expiry/read/password controls with visible consequences and native semantics.
- [x] Build an explicit burn/reveal gate, password entry, typed failure states, and lifecycle summary.
- [x] Build literal text, sanitized Markdown, source highlighting, safe blob-only image/text previews, and forced safe downloads.
- [x] Build local copy and QR sharing; label the complete URL as a bearer secret.
- [x] Build creator revoke UI around the delete capability.
- [x] Build light/dark/system theme, reduced motion, robust focus, live status, responsive/RTL layout, and dynamic document language.
- [x] Localize every user-visible core-flow string with English fallback; validate locale key parity.
- [x] Remove raw `{@html}` operator/translation sinks and externally configurable runtime images.
- [x] Replace Cryptgeon artwork/favicon with original Nyanbin SVG assets.

## Operations and documentation

- [x] Rebrand Rust crate/binary, workspace packages, Docker image, compose, examples, workflows, tests, docs, and release metadata.
- [x] Run the production image as a non-root user with a read-only-compatible filesystem.
- [x] Document ephemeral Valkey defaults, maxmemory policy, eviction, proxy trust, TLS, limits, and optional durability risks.
- [x] Keep Cryptgeon's original MIT notice and add clear Cryptgeon/PrivateBin provenance acknowledgements.
- [x] Generate a dependency and asset license inventory.
- [x] Write a comprehensive README with architecture, threat model, features, setup, configuration, CLI, API, deployment, development, security, and credits.
- [x] Add a private vulnerability reporting policy without embedding an unconfirmed contact URL.

## Verification

- [x] Rust formatting, static checks, and backend tests pass.
- [x] Frontend/CLI TypeScript checks and production builds pass.
- [x] Browser↔CLI E2E passes for all supported content, password, lifecycle, delete, and tamper flows.
- [x] Concurrent reveal test proves exactly the configured number of successes across two app instances.
- [x] Browser smoke test proves create → copy → explicit reveal → decrypt → consume, creator deletion, and expiry.
- [x] Desktop/mobile visual inspection confirms the blue kawaii theme without horizontal overflow.
- [x] Keyboard, focus, labels, live regions, reduced motion, and contrast are checked.
- [x] Browser network inspection confirms fragments, keys, passwords, delete capabilities, filenames, and plaintext never reach the server unexpectedly.
- [x] Production responses contain the documented CSP/security headers and no third-party runtime requests.
- [x] README commands and Compose deployment are exercised from a clean state.

### Verification evidence — 2026-08-14

- Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and 18 backend tests passed.
- Shared CLI/protocol: 20 Node tests passed, including immutable multi-origin clients, delayed password stdin, no-follow file I/O, canonical vectors, and tamper rejection.
- Browser/CLI: all 28 Chromium API, CLI, cross-client, file, lifecycle, password, revoke, expiry, Markdown, source, and tamper cases passed.
- Cross-replica experiment: 20 simultaneous reveal attempts split across two app instances sharing Valkey produced exactly 5 successes for `maxReads = 5` and 15 terminal misses.
- Production smoke: hardened image built; create/reserve/commit and explicit reveal/decrypt worked; expiry displayed in 2026; unchecked read cap sent explicit zero; request inspection found no fragment or private payload values.
- Locale validation: 12 catalogs each contain the same 153 leaf keys and placeholder sets; Arabic persisted with `lang=ar`, `dir=rtl`, native labels, and no horizontal overflow.
- Legal/release: the 195-row dependency inventory regenerated byte-identically, workflow YAML parsed, and CLI/runtime artifacts include all legal notices.


## Delivery

- [x] Create `mana/nyanbin` on `git.oh.rip` without exposing credentials in repository files or remote URLs.
- [x] Push the verified source and history.
- [x] Confirm the remote default branch, README, license, plan, and latest commit through the Forgejo API.
