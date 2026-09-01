# API Stability Policy

This document defines the compatibility guarantees for the Nyanbin public
HTTP API, described by [`openapi.yaml`](./openapi.yaml) and served as JSON at
`GET /api/openapi.json`.

## Two version axes

- **Wire protocol** (`protocol: 1` in requests, responses, and
  `GET /api/status`) governs **cryptographic compatibility**: envelope
  framing, key derivation, and everything the client-side protocol
  implementation (versioned GitHub release package `nyanbin-<version>.tgz`,
  export `./shared/protocol`) produces and consumes.
- **HTTP surface** (`info.version` in the OpenAPI document, currently `1`)
  governs **transport compatibility**: paths, methods, request/response
  fields, status codes, and error codes.

These are decoupled from each other and from the application release version
reported in `GET /api/status` (`version`).

## v1 guarantees: additive-only

Within the v1 surface:

- **Fields may be added, never removed or retyped.** New response fields may
  appear at any time; clients must ignore fields they do not recognize. A
  field's type, format, and meaning never change once published.
- **Paths are never renamed.** Existing paths, methods, and their success
  status codes remain stable.
- **The error-code enum is append-only.** New `code` values may be added;
  existing values are never removed or repurposed. Clients should treat an
  unrecognized code on a known status as a generic error of that status
  class. Error `message` strings are human-readable and **not** part of the
  stable surface.
- **Optional request fields may be added** with backwards-compatible
  defaults. Existing required fields never become stricter.

## Deprecation

If a capability must be phased out, it is first marked deprecated in the
OpenAPI document and the changelog, then dual-runs alongside its replacement
for a **minimum of six months** before any removal — and removal itself only
happens as part of a major surface change (below).

## Breaking changes

Any change that violates the guarantees above requires either:

- a **new major API path** (e.g. `/api/v2/...`) for transport-level breaks,
  leaving the v1 surface intact for the deprecation window, or
- a **protocol version bump** (`protocol: 2`) for cryptographic breaks,
  negotiated explicitly via the `protocol` field.

Temporary operational states — `503 writes_disabled`, `503 short_disabled`,
`507 storage_pressure`, and rate limiting — are not API changes and carry no
stability implications.
