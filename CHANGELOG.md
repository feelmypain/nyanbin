# Changelog

All notable changes to Nyanbin are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.5.4] - 2026-09-01

### Fixed

- Download the GitHub release tarball before installing the CLI so the documented command works even when npm policy disables direct remote-package fetches.

## [1.5.3] - 2026-09-01

### Fixed

- Return `409 reservation_mismatch` when a client repeats an already successful commit, instead of misclassifying it as a missing reservation.
- Return `413 payload_too_large` for a canonical envelope one decoded byte above the configured limit while preserving `400 invalid_envelope` for malformed framing.
- Document the versioned GitHub release artifact as the supported CLI installation source while npm publication credentials are unavailable, and verify the public artifact anonymously during tagged releases.
- Exclude liveness, readiness, status, and OpenAPI metadata endpoints from reference edge and origin throttles so application traffic cannot create false health failures.
- Keep rejection-counter ordering accepted by the CI-pinned Rust 1.95 Clippy toolchain and allow seven minutes for Playwright's managed server startup so a cold multi-stage Docker build cannot exhaust the readiness deadline.

## [1.5.2] - 2026-09-01

### Fixed

- Make `pnpm run check` build the workspace CLI dependency first, so frontend type checks resolve `nyanbin/shared` on a clean checkout rather than relying on a stale local `dist/` directory.
- Make the GitHub release workflow portable on GitHub-hosted runners: publish containers to GHCR by default, gate optional npm publication behind `PUBLISH_NPM`, and create releases with a GitHub-native action instead of an unsupported Forgejo action URL.

## [1.5.1] - 2026-09-01

### Fixed

- Restore the tracked `nyanbin-admin` binary source required by the declared Cargo target and production Docker image, and narrow the overly broad `bin/` ignore rule that omitted it from the v1.5.0 release. The restored CLI validates note IDs and pseudonymous bucket identifiers, requires explicit confirmation for revocation, and provides the documented `stats`, `switch`, `revoke`, `block`, `unblock`, and `resync` operations.

## [1.5.0] - 2026-09-01

### Added

- **Frozen public API contract.** The v1 HTTP surface is now described by a hand-authored OpenAPI 3.1 document (`docs/api/openapi.yaml`), rendered to canonical JSON at build time, embedded in the binary, and served verbatim at `GET /api/openapi.json`. A prerendered human-readable reference lives at `/docs/api`, and `docs/api/STABILITY.md` states the additive-only compatibility policy: fields are never removed or retyped, paths are never renamed, and the error-code enum is append-only. CI fails if the served contract drifts from the YAML source.
- **Per-operation rate limiting with `Retry-After`.** Reveal, info, delete, reserve, commit, short-create, and short-resolve are each metered separately per pseudonymous client bucket (IPv6 normalized to the operator-configured prefix, `/64` by default) plus a per-operation global ceiling that holds under address rotation. Every `429 rate_limited` response now carries an integer `Retry-After` header.
- **Storage occupancy guard.** With Valkey running `noeviction`, the app meters decoded envelope bytes against `NYANBIN_STORAGE_BUDGET_BYTES` (default 128 MiB): at 90% occupancy, commits larger than 64 KiB are refused with the new `507 storage_pressure`; at 98%, all commits are refused. Reads, reveals, and deletes always keep working. The meter self-heals against expiry drift on a `NYANBIN_STORAGE_METER_RESYNC_SECONDS` cadence without overwriting concurrent mutations.
- **Hourly per-client byte quotas.** Each client bucket may store at most `NYANBIN_BUCKET_BYTES_PER_HOUR` (default 64 MiB) of envelope bytes per hour; overflow returns `429 rate_limited` with `Retry-After` pointing at the hour boundary.
- **Operator kill switches and `nyanbin-admin`.** Valkey-backed switches (`writes_off`, `writes_small_only`, `short_off`, `resolve_hardened`) flip behavior at runtime without redeploys, surfacing as `503 writes_disabled` / `503 short_disabled` / `507 storage_pressure`. A short-code enumeration tripwire arms `resolve_hardened` automatically on a miss surge. The new `nyanbin-admin` binary ships in the app container: `stats`, `switch`, `revoke`, `block`, `unblock`, and `resync` — all output is aggregate and pseudonymous.
- **Opt-in CORS.** `NYANBIN_CORS_ORIGINS` enables cross-origin API access for an exact-origin allowlist or `*`; credentials are never allowed.
- **Abuse contact.** `NYANBIN_BRANDING_ABUSE_CONTACT` publishes an abuse email via `GET /api/status` branding, shown in the footer and API docs.

### Changed

- API error responses are now uniform everywhere: every non-2xx `/api` response, including axum-generated 404/405/413s, carries the `{code, message}` JSON body with a machine-readable append-only `code`.
- The backend crate is now a library with two binaries (`nyanbin`, `nyanbin-admin`).

## [1.4.2] - 2026-08-17

### Fixed

- The password gate on the note view no longer renders the input behind the in-field "Show" toggle. A page-level `form :global(button) { width: 100% }` rule was stretching every button in the form — including the reveal toggle inside the password field — across the whole input. The rule now targets only the form's direct child (the submit button); the toggle keeps its compact size across all themes and viewport widths.

## [1.4.1] - 2026-08-17

### Changed

- **Bare short links.** Password-protected notes are now keyed by the password alone (domain-separated PBKDF2-HMAC-SHA-256, 600,000 iterations, 16-byte envelope salt). Their share URLs are a bare `/note/{id}` and short URLs a bare `/s/{code}` — no `#{secret}` fragment — so a short code printed on paper or read aloud is the complete link. The note page gates on a required password field when the public info reports `passwordProtected`, and decryption falls back to the legacy secret+password key so links minted by v1.4.0 still open.
- The public info endpoint (`GET /api/notes/{id}`) now reports `passwordProtected` so clients can request the password before consuming a reveal.
- CLI: `open` on a bare password link works with `--password`/`--password-stdin`; `create --password` prints a bare link.

## [1.4.0] - 2026-08-17

### Added

- Optional first-party short links: password-protected notes can mint a 6-digit `/s/{code}` alias from the result view (`POST /api/notes/{id}/short`, resolved by `GET /api/short/{code}`). Codes require the creator delete capability, are idempotent per note, expire with the note, and die with revocation or the final read. Notes without a second-factor password are refused (`409 short_link_requires_password`) because a 6-digit code is guessable; the password keeps a discovered note sealed. The share URL keeps the secret in the fragment: `/s/{code}#{secret}`.
- Dedicated short-code rate limits: `NYANBIN_RATE_LIMIT_SHORT_CREATE_REQUESTS` (default 10/min per client) and `NYANBIN_RATE_LIMIT_SHORT_RESOLVE_REQUESTS` (default 60/min per client).
- The commit API accepts an optional `passwordProtected` flag so the server can gate short-code minting without learning the password.

### Changed

- The API client's `delete` method is renamed `deleteNote`; `createShort` and `resolveShort` are new.

## [1.3.1] - 2026-08-15

### Changed

- Envelope sizes, server limits, and over-limit amounts now display in human-readable units (e.g. "10.5 MB" instead of "10,485,760 bytes"), localized to the active language across all 12 locales.

## [1.3.0] - 2026-08-14

### Changed

- The dark theme is now a true black theme: near-black neutral canvas and surfaces with the kawaii blue accents kept for identity; the system-dark fallback matches exactly.

## [1.2.1] - 2026-08-14

### Removed

- The bin.oh.rip mirror is retired; nyan.ist is the only hosted instance. Documentation now references nyan.ist exclusively.

## [1.2.0] - 2026-08-14

### Added

- Official hosted instance at [nyan.ist](https://nyan.ist), served behind Cloudflare with strict origin TLS, HSTS, and HTTP/3.

### Changed

- The CLI default server is now the official instance `https://nyan.ist`; `--server` and `NYANBIN_SERVER` still select self-hosted instances.

## [1.1.1] - 2026-08-14

### Fixed

- The /about eyebrow now interpolates the active theme's color word instead of hardcoding "blue" in every locale.
- The default instance description is now empty on the backend, so the /about "This instance" text falls back to the localized `about.instance_default` string; operator-set `NYANBIN_BRANDING_DESCRIPTION` values are still shown verbatim.

## [1.1.0] - 2026-08-14

### Added

- Leave-confirm dialog that warns before abandoning an unsaved note.
- Kawaii red, green, and pink accent themes alongside the original blue, with a theme-aware hero title.
- Short expiry presets: 1, 5, and 30 minutes.
- Instance version reported by `GET /api/status` and displayed as a footer badge beside the GitHub source link.

### Changed

- "How the sealed envelope works" is now a proper card with an end-aligned chevron and a divided body.
- Footer version badge and submit help text received breathing room; README points at the official nyan.ist instance.

### Fixed

- System-dark color scheme now uses the same shadows as the explicit dark theme.

## [1.0.0] - 2026-08-14

### Added

- Nyanbin v1 authenticated AES-256-GCM envelope shared by the browser and Node.js CLI.
- Absolute expiry, optional atomic read caps, explicit reveal, and independent creator deletion capabilities.
- Plain text, source, Markdown, password, and encrypted multi-file payload support.
- Dependency-aware readiness and hardened ephemeral Valkey deployment defaults.
- Original blue kawaii interface and geometric cat branding.

### Changed

- Forked Cryptgeon's MIT-licensed Rust, Svelte, and CLI foundation into Nyanbin with a clean protocol and product cutover.
- Production containers now run as an unprivileged user and support read-only filesystems.
- Release artifacts publish only under repository-controlled npm and Forgejo container-registry credentials and namespaces.

### Removed

- Cryptgeon wire compatibility, Occulto adapters, legacy views/expiration modes, remote branding, and third-party runtime integrations.
- Discussions, never-expiring notes, external URL shorteners, operator HTML, and legacy translated deployment guides.

## Cryptgeon history

The entries below are retained as upstream change history and attribution.
## [2.4.0] - 2023-11-01

### Changed

- Removed HTML sanitation, display the original message as string
- Links are now displayed under the note in a separate section

## [2.3.1] - 2023-06-23

### Added

- #92: Endpoint (`/api/live/`) for checking health status.

## [2.3.0] - 2023-05-30

### Added

- New CLI 🎉.
- Russian language.
- Option for reducing note id size (`ID_LENGTH`).

### Changed

- Moved to monorepo.

### Changed

- Default port is now 8000, not 5000.
- Moved to generic encryption library `occulto`.

### Fixed

- Bad chinese language code.

### Security

- Updated dependencies.

## [2.1.0] - 2023-01-04

### Added

- QR Code to more easily copy and share links.

## [2.0.7] - 2022-12-26

### Changed

- Svelte Kit now stable 🎉

## [2.0.6] - 2022-11-12

### Fixed

- #66 Set minimum a view.

### Security

- Updated dependencies.

## [2.0.5] - 2022-11-04

### Fixed

- Docker build pipeline.

## [2.0.4] - 2022-10-29

### Added

- `THEME_PAGE_TITLE`.
- `THEME_FAVICON`.

## [2.0.3] - 2022-10-07

### Added

- Flag for verbosity.

### Fixed

- #58 Fixed bug in the max views frontend form.

## [2.0.2] - 2022-07-20

### Added

- Toasts for events.
- E2E Tests.
- Make backend more configurable.

## [2.0.1] - 2022-07-18

### Added

- Max file size on the client now.
- Loading information.

### Changed

- Changed encoding from hex to base64.
- Chinese language code.
- Notable speed improvements for big files.

## [2.0.0] - 2022-07-16

### Added

- Theming for logo and description text.

### Changed

- Moved to redis.
- New html sanitizing library.

## [2.0.0-rc.0] - 2022-07-15

### Added

- Theming for logo and description text.

### Changed

- Moved to redis.
- New html sanitizing library.

## [1.5.3] - 2022-06-07

### Changed

- Use the value from the `MEMCACHE` env variable in startup script.

## [1.5.2] - 2022-06-07

### Added

- Wait for script for memecached.

### Security

- Updated dependencies.

## [1.5.1] - 2022-05-15

### Fixed

- Remove double note content.

## [1.5.0] - 2022-05-14

### Added

- Links in notes are not highlighted and can be directly clicked #30.

## [1.4.1] - 2022-03-05

### Fixed

- Router in prod build.

## [1.4.0] - 2022-03-02

### Added

- Support for multiple languages.
- Select multiple files without removing already selected ones.
- Tooltip for copy action.
- Configure maximum views, expiration and advanced options for the server.

### Changed

- Use native SVGs instead of images.
- Update robots.txt file to allow only root.
- Stronger frontend types.

## [1.3.3] - 2022-01-03

### Fixed

- Bug fix due to dependency update.

## [1.3.2] - 2022-01-02

### Changed

- Dependencies updates.
- Folder structure.

## [1.3.1] - 2021-12-30

### Added

- Short explanation in the home page.

### Changed

- Explanation in about & readme.
- Shorten server ids from 512 to 256bit.

## [1.3.0] - 2021-12-22

### Added

- Option to set a custom size limit.
- Options to share files.

### Changed

- Don't delete note if time is not expired yet
- Use pnpm instead of npm.

## [1.2.0] - 2021-11-11

### Changed

- Switch to pnpm.

### Security

- Dependencies updated.

## [1.1.1] - 2021-05-17

### Fixed

- Height on big displays.
- About page.

## [1.1.0] - 2021-05-16

### Security

- Using hash `#` instead of path.

## [1.0.11] - 2021-05-08

### Added

- loading text.
- description for created notes about availability.

### Changed

- iterations from 100 to 100k.

### Fixed

- time based view bug.

## [1.0.10] - 2021-05-08

### Fixed

- API endpoint was not reachable.

## [1.0.9] - 2021-05-07

## Changed

- Removed a dependency.

## [1.0.8] - 2021-05-05

### Added

- Manual theme override option.

### Fixed

- Removed Arm builds for now.
- iOS style bugs.

## [1.0.7] - 2021-05-04

### Added

- Arm images.

## [1.0.6] - 2021-05-04

### Added

- Always use encryption with random passwords included links.

## [1.0.5] - 2021-05-03

### Fixed

- Typos.

## [1.0.4] - 2021-05-02

### Added

- From scratch docker image.

## [1.0.3] - 2021-05-02

### Fixed

- Higher default text area.
- Mobile touchups.

## [1.0.2] - 2021-05-02

### Fixed

- SVG Icons.

## [1.0.1] - 2021-05-02

### Added

- Dark mode support.

### Fixed

- Don't reload data on wrong password.

## [1.0.0] - 2021-05-02

Initial release.
